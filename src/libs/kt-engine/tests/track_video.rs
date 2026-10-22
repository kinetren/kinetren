//! R.62 (change B): muting a VIDEO TRACK — mute and solo.
//!
//! A muted track is not composited, but its PREFETCH REQUEST remains:
//! otherwise lifting mute would give a black frame until warm-up (invariant 4 —
//! we warm up exactly what we will read).

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::LaneKind::Video as V;
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 64, height: 36 };

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, lane: u32, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Two layers on ONE source, but with a different `source_in`: the synthetic frame is painted
/// by the TICK NUMBER, so the upper layer (track 0) reads frame 3000 (r = 184),
/// the lower one — frame 0 (r = 0).
/// So by a single pixel one can see which layer got into the composite.
const TOP_IN: i64 = 3_000;

fn project() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:silence"));
    let mut top = clip(1, 0, 1);
    top.source_in = Tick(TOP_IN);
    p.clips = vec![top, clip(2, 1, 1)];
    p.normalize();
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence))
            as Box<dyn MediaSource>,
    )]
}

fn render(p: &Project) -> (Vec<u8>, bool) {
    let mut facts = kt_engine::SourceFacts::default();
    for (id, s) in sources() {
        facts.observe(id, s.info());
    }
    let mut video = VideoRenderer::new_with_facts(p, &facts);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    // the REQUEST — what the engine asks for; we read exactly it
    for (media, pts, dq) in video.layers_at(Tick(0), Q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, Tick(0), Q)
}

/// What was REQUESTED: the pairs “source, frame” — by them one sees whether the prefetch
/// of the muted track went missing.
fn ordered(p: &Project) -> Vec<(u64, i64)> {
    let mut facts = kt_engine::SourceFacts::default();
    for (id, s) in sources() {
        facts.observe(id, s.info());
    }
    let video = VideoRenderer::new_with_facts(p, &facts);
    let mut v: Vec<(u64, i64)> =
        video.layers_at(Tick(0), Q).into_iter().map(|(m, t, _)| (m.0, t.0)).collect();
    v.sort();
    v.dedup();
    v
}

fn lum(f: &[u8]) -> u8 {
    f[0]
}

/// Test 735 (R.62): mute and solo of a video track.
#[test]
fn t735_video_mute_and_solo_leave_the_prefetch_order() {
    // scene: a bright layer on top — it is the one visible
    let base = project();
    let (f, complete) = render(&base);
    assert!(complete);
    let top_px = (TOP_IN & 0xff) as u8;
    assert_eq!(lum(&f), top_px, "scene: the TOP layer is in the frame");
    assert_eq!(ordered(&base), vec![(1, 0), (1, TOP_IN)], "scene: both frames are ordered");

    // MUTE of the upper track — the lower layer is in the frame, while the REQUEST is the old one
    let mut p = base.clone();
    p.track_at_mut(V, 0).unwrap().mute = true;
    let (f, complete) = render(&p);
    assert!(complete);
    assert_eq!(lum(&f), 0, "the muted track got into the composite");
    assert_eq!(
        ordered(&p),
        vec![(1, 0), (1, TOP_IN)],
        "the order for the muted track was lost — unmuting would give a black frame"
    );

    // SOLO — the set of ITS OWN kind: the composite contains exactly the solo tracks
    let mut p = base.clone();
    p.track_at_mut(V, 1).unwrap().solo = true;
    let (f, _) = render(&p);
    assert_eq!(lum(&f), 0, "in solo the bottom track must remain");
    assert_eq!(ordered(&p), vec![(1, 0), (1, TOP_IN)], "solo has no right to lose the order");

    // lifting mute returns the frame WITHOUT a miss: the request was there all along
    let mut p = base.clone();
    p.track_at_mut(V, 0).unwrap().mute = true;
    let _ = render(&p);
    p.track_at_mut(V, 0).unwrap().mute = false;
    let (f, complete) = render(&p);
    assert!(complete, "after unmuting the frame must be complete");
    assert_eq!(lum(&f), top_px);
}

/// Test 736 (R.62, change B): a track with a chain — ONE nested layer.
///
/// The claim the round was started for: the chain lands on the COMPOSITE of the track,
/// not written into each event. It is checked on a TRANSITION: in its window a per-clip chain
/// would apply the effect twice (once per half), while the track chain applies it exactly
/// once, on top of
/// the already mixed picture.
#[test]
fn t736_track_chain_covers_the_whole_composite() {
    use kt_model::{BlendMode, FxInstance};
    // one track, two events back to back with a transition between them
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:silence"));
    let mut a = clip(1, 0, 1);
    a.duration = Tick(6_000);
    let mut b = clip(2, 0, 1);
    b.start = Tick(6_000);
    b.duration = Tick(6_000);
    b.source_in = Tick(TOP_IN);
    p.clips = vec![a, b];
    p.normalize();
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(3_000),
        out_offset: Tick(3_000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let mid = Tick(6_000); // the middle of the transition window

    let render_at = |p: &Project, t: Tick| -> Vec<u8> {
        let mut facts = kt_engine::SourceFacts::default();
        for (id, s) in sources() {
            facts.observe(id, s.info());
        }
        let mut video = VideoRenderer::new_with_facts(p, &facts);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
            sources().into_iter().collect();
        for (media, pts, dq) in video.layers_at(t, Q) {
            let frame = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                kt_engine::video::video_cache_idx(pts),
                frame.into(),
            );
        }
        video.compose_from_cache(&cache, t, Q).0
    };

    // the CHAIN ON THE TRACK
    let mut on_track = p.clone();
    on_track.track_at_mut(V, 0).unwrap().fx = vec![FxInstance::Posterize { levels: 3 }];
    on_track.validate().unwrap();

    // the SAME chain, appended TO EACH EVENT
    let mut per_clip = p.clone();
    for c in per_clip.clips.iter_mut() {
        c.fx = vec![FxInstance::Posterize { levels: 3 }];
    }
    per_clip.validate().unwrap();

    let track = render_at(&on_track, mid);
    let clips = render_at(&per_clip, mid);
    assert_ne!(track, clips, "the track chain matched the per-clip one — so it is not on the composite");
    // and this is NOT “did nothing”: against a track without a chain the difference is there
    assert_ne!(track, render_at(&p, mid), "the track chain was not applied at all");

    // WITHOUT CONDITION — the old path: a track without a chain, level and mode
    // gives exactly the same as before R.62 (GOLDEN_VIDEO rests on this)
    let plain = render_at(&p, mid);
    let mut same = p.clone();
    same.track_at_mut(V, 0).unwrap().opacity = 1.0;
    same.track_at_mut(V, 0).unwrap().blend = BlendMode::Normal;
    assert_eq!(render_at(&same, mid), plain, "a track without properties changed the bytes");
}

/// Test 737 (R.62, change B): blend modes against the analytic
/// formula. Level and mode are DIFFERENT axes, both after the alpha; a hole in the
/// track's composite leaves the lower layers untouched.
#[test]
fn t737_blend_modes_match_the_formula() {
    use kt_engine::video::blend_ch;
    use kt_model::BlendMode as M;
    // the formulas — exactly those written in the spec
    for (b, a) in [(0.2f64, 0.6f64), (0.8, 0.3), (0.0, 1.0), (1.0, 0.0)] {
        assert_eq!(blend_ch(M::Normal, b, a), a);
        assert_eq!(blend_ch(M::Multiply, b, a), b * a);
        assert_eq!(blend_ch(M::Screen, b, a), b + a - b * a);
        assert_eq!(blend_ch(M::Add, b, a), (b + a).min(1.0));
        assert_eq!(blend_ch(M::Subtract, b, a), (b - a).max(0.0));
        assert_eq!(blend_ch(M::Darken, b, a), b.min(a));
        assert_eq!(blend_ch(M::Lighten, b, a), b.max(a));
        assert_eq!(blend_ch(M::Difference, b, a), (b - a).abs());
        let want = if b <= 0.5 { 2.0 * b * a } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - a) };
        assert_eq!(blend_ch(M::Overlay, b, a), want);
    }
    // the registry is complete: each kind has its own formula, and none is forgotten
    assert_eq!(M::ALL.len(), 9);
}
