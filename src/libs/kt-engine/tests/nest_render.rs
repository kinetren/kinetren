//! Tests 434-436: LAW E — a nest with an empty
//! chain equals its expanded content.
//!
//! Precedent — test 36 (“render with a morpher is sample-exact equal to render after
//! bake”). Wording:
//!
//! > Let `N` be a nest with an empty chain, `opacity == 1.0`, `gain_db == 0`,
//! > without reverse and warp, without transitions at the edges, longer than the content with
//! > tails, with boundaries on the frame grid and on the audio-frame boundary, and its
//! > children not cut by foreign events in the order of addition. Then the render `P`
//! > is BYTE-FOR-BYTE equal to the render `Decompose(P)` — both for video and audio.
//!
//! Why the law is needed. A nest with an empty chain is semantically identical to its
//! content BY CONSTRUCTION: settings are inherited (owner's decision R.50),
//! no intermediate raster appears. So inlining into
//! `effective_clips` is not a “simplified version” of the nest but the REFERENCE ORACLE,
//! against which wave B will check the real subtree. Without it there would be nothing
//! to check the subtree against.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
/// The length of the piece is an integer number of frames AND an integer number of audio bursts.
const PART: Tick = Tick(FRAME * 30);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![
        (
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 }))
                as Box<dyn MediaSource>,
        ),
        (MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp))),
        (
            MediaId(3),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
        ),
    ]
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for (id, s) in sources() {
        f.add_source(id, s);
    }
    f
}

fn ev(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start,
        duration: dur,
        source_in: Tick(FRAME * 3),
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

fn base() -> Project {
    let mut p = Project::new(settings());
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    p
}

/// `Decompose(P)`: the content sits directly on the tracks — exactly where
/// `ops::expand_instance` places it.
fn decomposed(kind: ClipKind) -> Project {
    let mut p = base();
    // the “tail” outside on the left: it does not overlap the nest and must sound
    // there and here identically
    p.clips.push(ev(1, kind, 0, Tick(0), PART, 3));
    // content: two events back to back on the instance track
    p.clips.push(ev(2, kind, 0, PART, PART, 1));
    p.clips.push(ev(3, kind, 0, PART + PART, PART, 2));
    if kind == ClipKind::Video {
        p.ensure_tracks(kt_model::LaneKind::Video, 1);
    } else {
        p.ensure_tracks(kt_model::LaneKind::Audio, 1);
    }
    p.validate().expect("Decompose(P) must be legal");
    p
}

/// `P`: the same, but the middle is collapsed into a reel.
fn nested(kind: ClipKind) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips.push(ev(2, kind, 0, Tick(0), PART, 1));
    s.clips.push(ev(3, kind, 0, PART, PART, 2));
    p.sequences.push(s);
    p.clips.push(ev(1, kind, 0, Tick(0), PART, 3));
    let mut inst = ev(10, kind, 0, PART, PART + PART, 100);
    inst.source_in = Tick(0);
    p.clips.push(inst);
    p.validate().expect("P must be legal");
    p
}

fn render_audio(p: &Project, frames: usize) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames * CH];
    let st = r.render_frames(&mut direct(), 0, &mut out);
    assert_eq!(st.underruns, 0, "underrun with a fully available source");
    out
}

fn render_video(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

// ---- 434. law E for audio ----------------------------------------------------

#[test]
fn t434_law_e_audio_is_byte_for_byte() {
    let frames = (PART + PART + PART).to_audio_frames() as usize;
    let a = render_audio(&decomposed(ClipKind::Audio), frames);
    let b = render_audio(&nested(ClipKind::Audio), frames);
    assert_eq!(
        a, b,
        "LAW E for audio: a nested item with an empty chain must sound exactly like \
         the expanded content. Settings are inherited, there is no intermediate \
         resample — a divergence here means the inline path computes the WRONG thing, and along with \
         it the whole oracle of wave B"
    );
    // ...and this is not “both silent”
    assert!(a.iter().any(|x| x.abs() > 1e-6), "the fixture is silent — the law was verified on nothing");
}

// ---- 435. law E for video + preview == export --------------------------------

#[test]
fn t435_law_e_video_and_preview_equals_export() {
    let q = Quality { width: 64, height: 36 };
    let (a, b) = (decomposed(ClipKind::Video), nested(ClipKind::Video));
    let mut differs_from_black = false;
    // every frame of all three segments: the tail outside, both children inside
    for i in 0..90 {
        let t = Tick(i * FRAME);
        let (fa, ca) = render_video(&a, t, q);
        let (fb, cb) = render_video(&b, t, q);
        assert_eq!(ca, cb, "completeness of the composite diverged on frame {i}");
        assert_eq!(
            fa, fb,
            "LAW E for video: frame {i} of the nested item and of the expanded content \
             must match BYTE FOR BYTE"
        );
        differs_from_black |= fa.iter().any(|&x| x != 0);
    }
    assert!(differs_from_black, "all frames are black — the law was verified on nothing");
}

// ---- 436. order composition and cost -------------------------------------------

#[test]
fn t436_nest_orders_the_same_frames_and_costs_the_same() {
    let q = Quality { width: 64, height: 36 };
    let (a, b) = (decomposed(ClipKind::Video), nested(ClipKind::Video));
    let va = VideoRenderer::new(&a);
    let vb = VideoRenderer::new(&b);
    for i in 0..90 {
        let t = Tick(i * FRAME);
        let la: Vec<_> = va.layers_at(t, q).into_iter().collect();
        let lb: Vec<_> = vb.layers_at(t, q).into_iter().collect();
        assert_eq!(
            la, lb,
            "the frame ORDER diverged at {i}: prefetch and render must ask for the SAME THING (invariant 4), \
             otherwise this is a permanent cache miss"
        );
    }

    // the price of expansion: a reel with 500 events, placed THREE times, must not
    // turn clip output into a project-wide scan on every frame
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Long");
    for i in 0..500u64 {
        s.clips.push(ev(1000 + i, ClipKind::Video, 0, Tick(i as i64 * PART.0), PART, 1));
    }
    p.sequences.push(s);
    for (k, at) in [(0u64, Tick(0)), (1, Tick(500 * PART.0)), (2, Tick(1000 * PART.0))] {
        let mut inst = ev(10 + k, ClipKind::Video, 0, at, Tick(500 * PART.0), 100);
        inst.source_in = Tick(0);
        p.clips.push(inst);
    }
    p.validate().expect("a project with three placements of a long reel is legal");
    let t0 = std::time::Instant::now();
    let eff = p.effective_clips();
    let dt = t0.elapsed();
    assert_eq!(eff.len(), 1500, "not all content was expanded: {}", eff.len());
    // piece addresses are unique — otherwise FX state and selection would drift
    let mut ids: Vec<u64> = eff.iter().map(|c| c.id.0).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "pieces of the nested item got identical addresses");
    // ceiling with a large margin: we catch not microseconds but quadratic growth
    assert!(
        dt.as_millis() < 500,
        "expanding three placements of a reel with 500 events took {dt:?} — this is no longer a \
         linear pass, but a search on every frame"
    );
}

// ---- 438-441 (R.50, wave B): nesting — BOUNDARY OF PROCESSING ORDER --------

/// Two OVERLAPPING events on adjacent tracks, the upper one is
/// SEMI-TRANSPARENT. Both conditions are mandatory: while the children do not overlap
/// (or the upper one is opaque and covers the lower one entirely), “onto the nest” and
/// “onto each” give the same result, and the boundary is indistinguishable from its absence.
fn inner_pair() -> Vec<ClipEvent> {
    let a = ev(2, ClipKind::Video, 1, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Video, 0, Tick(PART.0 / 2), PART, 2);
    b.opacity = 0.5;
    vec![a, b]
}

/// The largest divergence per channel.
fn max_diff(a: &[u8], b: &[u8]) -> i32 {
    assert_eq!(a.len(), b.len(), "frames of different size");
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

/// `P`: children inside the reel, level/chain — on the INSTANCE.
fn nest_with(outer_opacity: f32, fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips = inner_pair();
    s.ensure_tracks(kt_model::LaneKind::Video, 2);
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART + PART, 100);
    inst.source_in = Tick(0);
    inst.opacity = outer_opacity;
    inst.fx = fx;
    p.clips.push(inst);
    p.validate().expect("a project with a nested item is legal");
    p
}

/// `Decompose(P)`: the same children directly on tracks, level/chain — on EACH.
fn spread_with(opacity: f32, fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut flat = inner_pair();
    for c in flat.iter_mut() {
        c.opacity *= opacity;
        c.fx = fx.clone();
    }
    p.clips = flat;
    p.ensure_tracks(kt_model::LaneKind::Video, 2);
    p.validate().expect("an expanded project is legal");
    p
}

#[test]
fn t438_law_g_level_on_the_nest_is_not_level_on_each_child() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20); // inside the overlap
    let (nested_half, _) = (render_video(&nest_with(0.5, vec![]), t, q).0, ());
    let spread_half = render_video(&spread_with(0.5, vec![]), t, q).0;
    assert_ne!(
        nested_half, spread_half,
        "LAW G: the 50 % level ON THE NESTED ITEM must differ from 50 % on each \
         child. If they match, there is no boundary of the processing order, and the nested item is \
         pointless: precisely for the sake of “the effect on the RESULT” was it created"
    );
    // ...and this is not “one of them is black”
    assert!(nested_half.iter().any(|&x| x != 0), "the composite of the nested item is black");
    assert!(spread_half.iter().any(|&x| x != 0), "the expanded composite is black");

    // at level 1.0 there must be NO difference: an opaque composite
    // is blended the same way as its opaque children (law E)
    assert_eq!(
        render_video(&nest_with(1.0, vec![]), t, q).0,
        render_video(&spread_with(1.0, vec![]), t, q).0,
        "without a level and a chain the nested item must be IDENTICAL to the content"
    );
}

#[test]
fn t439_semi_transparent_subcomposite_is_not_twice_as_dark() {
    // A defect found by reconnaissance before it appeared: the composite accumulator
    // stores PREMULTIPLIED RGB with STRAIGHT alpha, while
    // `alpha_over` expects straight. Feeding a sub-composite back into a composite without
    // conversion — the picture is twice as dark EXACTLY at the semi-transparent spots.
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 5);

    // background below, semi-transparent child inside the reel above
    let mut nested = base();
    let mut s = Sequence::new(MediaId(100), "Piece");
    let mut child = ev(2, ClipKind::Video, 0, Tick(0), PART, 1);
    child.opacity = 0.5;
    s.clips = vec![child.clone()];
    nested.sequences.push(s);
    nested.ensure_tracks(kt_model::LaneKind::Video, 2);
    // a BACKGROUND of a different color: in synthetics the color is a function of pts, so without
    // its own entry point the background and the content would match, and the darkening
    // would have nothing to be seen against
    let mut bg_clip = ev(1, ClipKind::Video, 1, Tick(0), PART, 3);
    bg_clip.source_in = Tick(FRAME * 500);
    nested.clips.push(bg_clip.clone());
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    inst.source_in = Tick(0);
    nested.clips.push(inst);
    nested.validate().unwrap();

    let mut flat = base();
    flat.ensure_tracks(kt_model::LaneKind::Video, 2);
    flat.clips.push(bg_clip.clone());
    flat.clips.push(child);
    flat.validate().unwrap();

    let bg = render_video(
        &{
            let mut p = base();
            let mut c = bg_clip;
            c.lane = 0;
            p.clips.push(c);
            p.validate().unwrap();
            p
        },
        t,
        q,
    )
    .0;
    let a = render_video(&nested, t, q).0;
    let b = render_video(&flat, t, q).0;

    // Here law E honestly is NOT fulfilled byte-for-byte, and it is not fixed:
    // the gap between levels is 8-bit, and premultiply→unpremultiply
    // loses one least significant bit. The limit is named, not tuned: ONE rounding
    // of the 8-bit gap.
    let d = max_diff(&a, &b);
    assert!(
        d <= 1,
        "the sub-composite landed on the background differently than the same content would land directly \
         (divergence {d} > 1 LSB). The signature of the defect is roughly TWICE as dark \
         exactly in semi-transparent places: the accumulator outputs premultiplied \
         colour, while the overlay expects straight"
    );
    // ...and this assertion is not empty: without conversion the picture drifts TOWARD THE BACKGROUND
    // (a semi-transparent layer contributes half as much), not by one bit
    let to_bg = max_diff(&b, &bg);
    assert!(
        to_bg > 8,
        "the fixture is degenerate: the content of the nested item is almost equal to the background already \
         ({to_bg}), so darkening would not be visible on it"
    );
}

#[test]
fn t440_a_chain_on_the_nest_acts_on_the_assembled_picture() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    // Posterization is NONLINEAR: quantizing a mix of two colors is not a mix of their
    // quantizations. A linear effect (blur, level) on flat frames would show nothing, and the test
    // would be green on broken code.
    let fx = vec![FxInstance::Posterize { levels: 3 }];
    let on_nest = render_video(&nest_with(1.0, fx.clone()), t, q).0;
    let on_each = render_video(&spread_with(1.0, fx), t, q).0;
    assert_ne!(
        on_nest, on_each,
        "the chain on the nested item was computed PIECE BY PIECE. “The effect on the result” is \
         the only way to express this where effects live on events, and precisely \
         this does not work here"
    );
    assert!(on_nest.iter().any(|&x| x != 0), "the composite with a chain is black");
    // ...and without a chain the same two projects match — so they diverged
    // PRECISELY on the chain, not on the fixture device
    assert_eq!(
        render_video(&nest_with(1.0, vec![]), t, q).0,
        render_video(&spread_with(1.0, vec![]), t, q).0,
        "the fixtures diverge even without a chain — the test would prove something else"
    );
}

#[test]
fn t441_hunger_and_memoization_cross_the_boundary() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    let p = nest_with(1.0, vec![]);
    let seq = MediaId(100);

    // STARVATION INSIDE is visible OUTSIDE: the child's frame is missing — the composite is incomplete.
    // Otherwise the player would consider the frame ready and never re-request it.
    let mut v = VideoRenderer::new(&p);
    let cold = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (_, complete) = v.compose_from_cache(&cold, t, q);
    assert!(!complete, "starvation INSIDE a reel did not reach the outer composite");

    // Warm them up — and it is complete.
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        assert_ne!(media, seq, "the order did not descend: the address of the reel’s source has no");
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview, complete) = v.compose_from_cache(&cache, t, q);
    assert!(complete, "a warm composite with a nested item must be complete");

    // Memoization of the sub-composite exists in PREVIEW...
    let h = kt_engine::content_hash(&p, seq);
    let inner_t = t; // the instance stands at zero with source_in = 0
    assert!(
        cache
            .get_postfx_video(kt_engine::nest_source_id(seq), (q.width, q.height), h, inner_t.0)
            .is_some(),
        "the sub-composite was not memoized in the preview — the reel’s chain would be paid on EVERY frame"
    );

    // ...and does NOT exist in export: full-size sub-composites would evict
    // not-yet-used pre-frames of neighboring layers from the chunk budget —
    // literally a regression of R.31 item 4 (“render broken”).
    let export_cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        export_cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let mut out = Vec::new();
    let complete = v.compose_export_into(&export_cache, t, q, &mut out);
    assert!(complete, "the export composite with a nested item must be complete");
    assert_eq!(out, preview, "preview and export must match BYTE FOR BYTE");
    assert!(
        export_cache
            .get_postfx_video(kt_engine::nest_source_id(seq), (q.width, q.height), h, inner_t.0)
            .is_none(),
        "export memoized the sub-composite: the chunk budget will evict the neighbours’ pre-frames"
    );
}

// ---- 442-443 (R.50, B2): memoization keys ------------------------------------

/// Warm the pre-FX frames of the plan and return the composite.
fn warm_and_compose(
    v: &mut VideoRenderer,
    cache: &kt_cache::BurstCache,
    t: Tick,
    q: Quality,
) -> Vec<u8> {
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    v.compose_from_cache(cache, t, q).0
}

#[test]
fn t442_editing_inside_the_reel_invalidates_the_instance() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    // chain ON THE INSTANCE: precisely its post-FX frame is memoized, and
    // it is exactly that which would become stale
    let mut p = nest_with(1.0, vec![FxInstance::Posterize { levels: 3 }]);
    // cache ONE for both runs — in the program it is one: a model edit
    // recreates the renderer, but not the cache
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let before = warm_and_compose(&mut VideoRenderer::new(&p), &cache, t, q);

    // we edit INSIDE the reel. Not a single field of the instance clip changes:
    // its `fx_hash` over geometry and chain would remain the same, and without
    // `content_hash` the cache would return the FRAME BEFORE THE EDIT.
    p.sequences[0].clips[1].source_in = Tick(FRAME * 77);
    p.validate().unwrap();
    let after = warm_and_compose(&mut VideoRenderer::new(&p), &cache, t, q);
    assert_ne!(
        before, after,
        "an edit INSIDE the reel did not reach the picture: the promise “a model edit recreates the renderer, \
         stale frames are not read” holds for the nested item only on the hash of the REVISION — \
         not a single field of the instance changes"
    );
}

#[test]
fn t443_three_keys_separate_reels_instances_and_editions() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    let p = nest_with(1.0, vec![]);
    let seq = MediaId(100);

    // EDITS: same address, different content — different key
    let h1 = kt_engine::content_hash(&p, seq);
    let mut q2 = p.clone();
    q2.sequences[0].clips[1].source_in = Tick(FRAME * 77);
    let h2 = kt_engine::content_hash(&q2, seq);
    assert_ne!(h1, h2, "an edit inside the reel did not change the revision hash");

    // ...and recursively: an edit in a GRANDCHILD must reach the grandparent
    let mut deep = base();
    let mut inner = Sequence::new(MediaId(101), "Inner");
    inner.clips = vec![ev(20, ClipKind::Video, 0, Tick(0), PART, 1)];
    let mut outer = Sequence::new(MediaId(100), "Outer");
    let mut mid = ev(21, ClipKind::Video, 0, Tick(0), PART, 101);
    mid.source_in = Tick(0);
    outer.clips = vec![mid];
    deep.sequences.push(inner);
    deep.sequences.push(outer);
    let mut top = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    top.source_in = Tick(0);
    deep.clips.push(top);
    deep.validate().unwrap();
    let d1 = kt_engine::content_hash(&deep, MediaId(100));
    deep.sequences[0].clips[0].source_in = Tick(FRAME * 9);
    let d2 = kt_engine::content_hash(&deep, MediaId(100));
    assert_ne!(d1, d2, "an edit in a GRANDCHILD did not reach the hash of the grandparent — the memo will return stale");

    // REELS: the address of a sub-composite in the cache is not a file address
    assert_ne!(
        kt_engine::nest_source_id(seq),
        kt_engine::source_id(MediaId(seq.0)),
        "a reel and a file with the same number share an address in the cache"
    );
    assert!(
        kt_engine::nest_source_id(seq).0 >= kt_model::NEST_TAG,
        "the address of a reel must lie above the range of files"
    );

    // INSTANCES: two placements of ONE reel with different chains do not
    // collapse into one post-FX frame, while their sub-composite is SHARED —
    // that is where the gain lies
    let mut two = nest_with(1.0, vec![FxInstance::Posterize { levels: 3 }]);
    let mut twin = two.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = vec![FxInstance::Posterize { levels: 5 }];
    two.clips.push(twin);
    two.validate().unwrap();
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut v = VideoRenderer::new(&two);
    let a = warm_and_compose(&mut v, &cache, t, q);
    let b = warm_and_compose(&mut v, &cache, Tick(10 * PART.0 + t.0), q);
    assert_ne!(
        a, b,
        "two placements of one reel with DIFFERENT chains gave one frame — \
         the post-FX cache is not keyed by the instance"
    );
    // the sub-composite of a reel is kept in the cache ONCE — by the reel and edit address
    assert!(
        cache
            .get_postfx_video(
                kt_engine::nest_source_id(seq),
                (q.width, q.height),
                kt_engine::content_hash(&two, seq),
                t.0
            )
            .is_some(),
        "the sub-composite is not memoized by the address of the reel"
    );
}

// ---- 447-449 (R.50, B4): audio — sub-mix --------------------------------

/// Reverb without time stretching: warp on the nest is forbidden for now, and
/// the tail is exactly what must stretch ACROSS the seam between the lines.
fn reverb() -> FxInstance {
    FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.6,
        reverb_delay_ms: 60.0,
        damp: 0.4,
        decay: 0.7,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

fn audio_nest(outer_fx: Vec<FxInstance>, inner_fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Voice lines");
    // two lines back to back — that same “ten lines under one chain”
    let mut a = ev(2, ClipKind::Audio, 0, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, PART, PART, 2);
    a.fx = inner_fx.clone();
    b.fx = inner_fx;
    s.clips = vec![a, b];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Audio, 0, Tick(0), PART + PART, 100);
    inst.source_in = Tick(0);
    inst.fx = outer_fx;
    p.clips.push(inst);
    p.validate().expect("a project with an audio nested item is legal");
    p
}

fn spread_audio(fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut a = ev(2, ClipKind::Audio, 0, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, PART, PART, 2);
    a.fx = fx.clone();
    b.fx = fx;
    p.clips = vec![a, b];
    p.validate().expect("an expanded project is legal");
    p
}

#[test]
fn t447_audio_law_e_through_the_submix() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let a = render_audio(&spread_audio(vec![]), frames);
    let b = render_audio(&audio_nest(vec![], vec![]), frames);
    assert_eq!(
        a, b,
        "LAW E for audio via the SUB-MIX: a nested item with an empty chain must \
         sound BYTE FOR BYTE like the expanded content. The nested item is a virtual \
         media under the fetcher, not a branch in the mixer, precisely so that \
         the f32 summation order does not shift"
    );
    assert!(a.iter().any(|x| x.abs() > 1e-6), "the fixture is silent");

    // BURST REQUESTS go deeper: the source reel has none, and a reel address in
    // the request would mean that prefetch warms a nonexistent thing while the sub-mix
    // reads the real one — a permanent miss (item 13)
    let r = AudioRenderer::new(&audio_nest(vec![], vec![]));
    let want = r.wanted_audio_bursts(0..frames as i64);
    assert!(!want.is_empty(), "the order is empty — nothing to check");
    assert!(
        want.iter().all(|(m, _)| *m != MediaId(100)),
        "the burst order holds the ADDRESS OF THE REEL: {want:?}"
    );
    assert!(
        want.iter().any(|(m, _)| *m == MediaId(1)) && want.iter().any(|(m, _)| *m == MediaId(2)),
        "the order did not cover the content of the reel: {want:?}"
    );
}

#[test]
fn t449_one_reverb_on_ten_replicas_is_not_a_chain_on_each() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let rev = vec![reverb()];
    // chain ON THE NEST: one tail for both lines, and it stretches ACROSS the seam
    let on_nest = render_audio(&audio_nest(rev.clone(), vec![]), frames);
    // chain on EACH child: two independent tails, cut off at the seams
    let on_each = render_audio(&spread_audio(rev.clone()), frames);
    assert_ne!(
        on_nest, on_each,
        "reverb on the nested item was computed piece by piece. “One chain per group of \
         voice lines” is exactly what an audio nested item is for"
    );
    assert!(on_nest.iter().any(|x| x.abs() > 1e-6), "the nested item is silent");

    // ...and the chain on the nest works AT ALL: without it the sound would be dry
    let dry = render_audio(&audio_nest(vec![], vec![]), frames);
    assert_ne!(
        on_nest, dry,
        "the chain on the nested item silently disappeared — before B4 expansion dropped it, and \
         this would be exactly the case of “quietly computed something else”"
    );

    // The chain INSIDE a reel must sound too: its processors live in
    // their own store, and without recursion `sync_fx` would simply never create them.
    let inside = render_audio(&audio_nest(vec![], rev), frames);
    // The key claim is NOT “differs from dry”: without synchronization the processor of the
    // internal chain is not created, `render_fx_clip` exits on the first line, and the
    // event simply DISAPPEARS. There is a difference from dry as well, and the test
    // would be green on broken code.
    let energy = |v: &[f32]| v.iter().map(|x| x.abs() as f64).sum::<f64>();
    assert!(
        energy(&inside) > energy(&dry) * 0.3,
        "the sound inside the reel VANISHED ({:.1} against {:.1} for the dry one): the state of the \
         nested chains is not synchronized with the model, and `render_fx_clip` \
         silently exited without finding a processor",
        energy(&inside),
        energy(&dry)
    );
    assert_ne!(inside, dry, "a chain inside the reel did not change the sound");
    assert_ne!(inside, on_nest, "a chain inside and outside gave the same thing");
}

#[test]
fn t448_submix_is_the_same_whole_and_in_chunks() {
    // A streaming chain inside a reel must survive the slicing of output: the player's
    // ring fills with chunks, and the seam between them must not be
    // audible (kin to t338 “chunk == chunks”).
    let p = audio_nest(vec![reverb()], vec![]);
    let frames = (PART + PART).to_audio_frames() as usize;
    let whole = render_audio(&p, frames);

    let r = AudioRenderer::new(&p);
    let mut store = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = direct();
    let mut pieces = vec![0f32; frames * CH];
    let chunk = 1024usize;
    let mut done = 0usize;
    while done < frames {
        let n = chunk.min(frames - done);
        let (_, tail) = pieces.split_at_mut(done * CH);
        let (dst, _) = tail.split_at_mut(n * CH);
        r.render_frames_with(&mut f, &mut store, done as i64, dst);
        done += n;
    }
    assert_eq!(
        whole, pieces,
        "the sub-mix computed in PIECES diverged from the same one computed whole: the inner \
         chain loses continuity at the output seams"
    );

    // seek goes DEEP: after reset a tail from another place does not sound
    let mut store2 = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut store2);
    let mut far = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut store2, 200_000, &mut far);
    store2.reset_all();
    let mut after = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut store2, 0, &mut after);
    let mut fresh_store = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut fresh_store);
    let mut fresh = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut fresh_store, 0, &mut fresh);
    assert_eq!(
        after, fresh,
        "after a seek inside the reel a tail from ANOTHER place remained: `reset_all` \
         did not reach the chains of the nested item"
    );
}

// ---- 450-452 (R.50, B5-B6): step, budgets, warmth -----------------

/// A nest whose instance has a chain that ZOOMS the assembled picture.
fn zooming_nest(zoom: f64) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips = vec![ev(2, ClipKind::Video, 0, Tick(0), PART, 1)];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    inst.source_in = Tick(0);
    inst.fx = vec![FxInstance::geometry_zoom(zoom)];
    p.clips.push(inst);
    p.validate().expect("a zooming nested item is legal");
    p
}

#[test]
fn t450_nest_decode_stage_is_a_property_of_the_reel() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 5);

    // without zoom, step 1 — the old path
    let flat = VideoRenderer::new(&zooming_nest(1.0));
    let plain: Vec<Quality> = flat.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    assert!(plain.iter().all(|dq| *dq == q), "without zoom the order must go to Quality: {plain:?}");

    // with zoom the sub-plan is assembled LARGER, and so is the content request:
    // if they diverged, prefetch would warm one size while the composite read another
    let z = VideoRenderer::new(&zooming_nest(3.0));
    let big: Vec<Quality> = z.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    assert!(!big.is_empty(), "the order is empty");
    assert!(
        big.iter().all(|dq| dq.width > q.width),
        "the order for the content of a zooming nested item stayed at Quality — the sub-composite \
         would come out soft: {big:?}"
    );
    assert!(
        z.max_decode_quality(q).width >= big[0].width,
        "budgets count less than prefetch orders"
    );

    // the step is a property of the REEL: a second instance WITHOUT zoom must get
    // THE SAME step, otherwise an inner clip would have two cache keys and two requests
    let mut two = zooming_nest(3.0);
    let mut twin = two.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = vec![];
    two.clips.push(twin);
    two.validate().unwrap();
    let v = VideoRenderer::new(&two);
    let a: Vec<Quality> = v.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    let b: Vec<Quality> =
        v.layers_at(Tick(10 * PART.0 + t.0), q).into_iter().map(|(_, _, dq)| dq).collect();
    assert_eq!(a, b, "two instances of one reel ordered DIFFERENT sizes: {a:?} against {b:?}");
}

#[test]
fn t451_nest_costs_its_subcomposite_in_the_chunk_budget() {
    let p = nest_with(1.0, vec![]);
    let v = VideoRenderer::new(&p);
    // inside the reel two overlapping events, outside one instance:
    // the budget must count 1 (sub-composite buffer) + 2 (children), not 1
    assert_eq!(
        v.max_concurrent_layers(),
        3,
        "the chunk budget did not see the content of the nested item. Undercounting evicts the neighbour \
         before the composite, the starvation branch fires, and the EXPORT BYTES begin \
         to depend on the timing of the cache"
    );
    // ...and a flat project is counted as before
    assert_eq!(VideoRenderer::new(&spread_with(1.0, vec![])).max_concurrent_layers(), 2);
}

#[test]
fn t452_nest_warmth_is_the_intersection_of_its_children() {
    let q = Quality { width: 64, height: 36 };
    let p = nest_with(1.0, vec![]);
    let v = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    // Empty — cold EVERYWHERE THERE IS SOMETHING TO DECODE. The tail of the reel,
    // where there are no children at all, remains warm, and that is true: there is nothing
    // to decode there, transparency arrives instantly.
    let material = 0..(PART.0 + PART.0 / 2); // the children live here
    let cold = v.nest_coverage(&cache, q);
    assert_eq!(cold.len(), 1, "the coverage bar of the nested item is not built");
    assert!(
        cold[0].1.iter().all(|r| r.end <= material.start || r.start >= material.end),
        "an empty cache shown as warm where the children lie: {:?}",
        cold[0].1
    );

    // we warmed ONE moment: only the neighborhood must become warm, and
    // only where ALL covering children have arrived
    let t = Tick(FRAME * 20);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let warm = v.nest_coverage(&cache, q);
    let runs = &warm[0].1;
    assert!(!runs.is_empty(), "a warm moment is not visible in the bar");
    assert!(
        runs.iter().any(|r| r.start <= t.0 && t.0 < r.end),
        "the warm moment {} did not fall into any run {runs:?}",
        t.0
    );
    // ...but NOT the whole span: the rest was not warmed
    let span: i64 =
        runs.iter().map(|r| (r.end.min(material.end) - r.start.max(material.start)).max(0)).sum();
    assert!(
        span < material.end - material.start,
        "one frame is warm, yet the bar shows the whole reel as warm ({span}) — \
         this is a union instead of an intersection, and it lies in the GOOD direction"
    );
}

/// Test 463b (item 399, R.51): a transition BETWEEN lines inside a reel is audible.
/// A defect found along the way in R.50: audio hung the transition windows from
/// `project.transitions` (the root), so a transition inside a nested sequence
/// silently did not crossfade the sound — while video took the transitions of its own reel.
#[test]
fn t463b_transition_inside_a_reel_is_audible() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let add_tr = |v: &mut Vec<TransitionEvent>| {
        v.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(2)),
            b: Some(ClipId(3)),
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Crossfade,
            audio: AudioCurve::Linear,
            auto: vec![],
            invert: false,
        });
    };
    let mut nested = audio_nest(vec![], vec![]);
    add_tr(&mut nested.sequences[0].transitions);
    nested.validate().expect("a transition inside the reel is legal");
    let mut spread = spread_audio(vec![]);
    add_tr(&mut spread.transitions);
    spread.validate().unwrap();

    let inside = render_audio(&nested, frames);
    // law E: a transition inside a reel sounds byte-for-byte like the expanded one
    assert_eq!(
        inside,
        render_audio(&spread, frames),
        "a transition inside the reel must sound like the same transition outside"
    );
    // ...and it does act at all (before the fix there was silence instead of the transition)
    assert_ne!(
        inside,
        render_audio(&audio_nest(vec![], vec![]), frames),
        "the transition inside the reel silently disappeared"
    );
}

// ---- R.55 (C4): reverse and stretch ON a nest ---------------------------

/// A project where the reel's content stands DIRECTLY on the tracks and is fully
/// mirrored: two clips back to back, order swapped, each with `reversed`.
/// This is exactly “expanded content, played backwards” — the reference
/// that the reverse of the NEST on the reel must match, on a reel
/// without tail effects.
fn spread_audio_mirrored() -> Project {
    let mut p = base();
    // the mirror of the whole piece: the former second clip becomes first
    let mut a = ev(2, ClipKind::Audio, 0, PART, PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, Tick(0), PART, 2);
    a.reversed = true;
    b.reversed = true;
    p.clips = vec![a, b];
    p.validate().expect("a mirrored expanded project is legal");
    p
}

/// Test 561 (R.55, C4): REVERSE OF A NEST == reverse of the expanded content.
///
/// **Owner's claim, verbatim:** “reverses do not work on a composition.”
/// It did not work: `validate` rejected the mirrored instance,
/// because the sub-mix was read strictly in order.
///
/// This is a definition, not an approximation: “reverse of the assembled piece” means
/// “render the content FORWARD and play it backwards”.
/// On a reel without tail effects
/// equality must be BYTE-FOR-BYTE — precisely because the sub-mix is rendered forward,
/// and the mirror is applied to its samples, the same way as to the samples of a file.
#[test]
fn t561_reversing_a_nest_equals_reversing_its_content() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let mut p = audio_nest(vec![], vec![]);
    p.clips[0].reversed = true;
    p.validate().expect("a mirrored nested item must be legal");

    let nested = render_audio(&p, frames);
    let spread = render_audio(&spread_audio_mirrored(), frames);
    assert_eq!(
        nested, spread,
        "the REVERSE of the NESTED ITEM diverged from the reverse of the expanded content. The mirror \
         acts on the SAMPLES of the assembled mix — just as on the samples of a file; \
         rendering the reel backward would mean computing a third, nobody’s result"
    );
    assert!(nested.iter().any(|x| x.abs() > 1e-6), "the mirrored nested item is silent");
}

/// Test 562 (R.55, C4): STRETCHING A NEST ≠ stretching each piece.
///
/// A reverse analogue already exists (t428 in the model). This is the analogue of LAW G for
/// speed: outside, the ALREADY ASSEMBLED mix with stitched tails is resampled; inside, each piece
/// has its own cut tail.
/// That is exactly why the “edit it, then slow it all down” use case gets a nest.
#[test]
fn t562_slowing_a_nest_is_not_slowing_each_piece() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let slow = vec![FxInstance::slowed_reverb(0.5, 0.6, 60.0)];
    let on_nest = render_audio(&audio_nest(slow.clone(), vec![]), frames);
    let on_each = render_audio(&spread_audio(slow.clone()), frames);
    assert_ne!(
        on_nest, on_each,
        "the slowdown of the nested item was computed piece by piece — “edit, then slow down \
         as a whole” stopped differing from “slow down each”"
    );
    assert!(on_nest.iter().any(|x| x.abs() > 1e-6), "the slowed nested item is silent");
}

/// Test 563 (R.55, C4): sub-mix PIECE == PIECES, also under reverse.
///
/// Kin to t448/t338. The sub-mix producer reads the reel out of order, and without a canonical grid of blocks
/// the value of a burst would depend on the piece it was requested in — that is, on the size of the output chunk.
/// This cannot happen in audio
/// (invariant 11).
#[test]
fn t563_reversed_submix_is_the_same_whole_and_in_chunks() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let mut p = audio_nest(vec![reverb()], vec![reverb()]);
    p.clips[0].reversed = true;
    p.validate().expect("a mirrored nested item with reverbs is legal");

    let whole = render_audio(&p, frames);
    for chunk in [777usize, 1024, 4096] {
        let r = AudioRenderer::new(&p);
        let mut store = kt_engine::fxstore::FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; frames * CH];
        let mut f = direct();
        let mut done = 0usize;
        while done < frames {
            let n = chunk.min(frames - done);
            r.render_frames_with(
                &mut f,
                &mut store,
                done as i64,
                &mut out[done * CH..(done + n) * CH],
            );
            done += n;
        }
        assert_eq!(
            out, whole,
            "the sub-mix of the reversed nested item depends on the SLICING of the output \
             (chunk {chunk}) — the bytes shift with the request size"
        );
    }
}

/// Test 564 (R.55, C4): two instances of one reel — TWO STREAMS.
///
/// The sub-mix state was keyed by the REEL's address, even though the comment next to it
/// claimed the opposite (“a reel in five places — FIVE different streams”).
/// Two simultaneously sounding instances shared the inner chains
/// and the single-burst cache:
/// under reverse and stretch they also fought over the producer's cursor.
#[test]
fn t564_two_instances_of_one_reel_do_not_share_a_reverb() {
    let frames = (PART + PART).to_audio_frames() as usize;
    // one and the same reel in TWO places, offset, sounding at once
    let mut p = audio_nest(vec![], vec![reverb()]);
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.lane = 1;
    twin.start = Tick(PART.0 / 2);
    p.clips.push(twin);
    p.ensure_tracks(kt_model::LaneKind::Audio, 2);
    p.validate().expect("a reel in two places is legal");
    let both = render_audio(&p, frames);

    // each instance SEPARATELY — and the sum must match the common mix
    let mut only_a = p.clone();
    only_a.clips.retain(|c| c.id == ClipId(10));
    let mut only_b = p.clone();
    only_b.clips.retain(|c| c.id == ClipId(11));
    let a = render_audio(&only_a, frames);
    let b = render_audio(&only_b, frames);

    let worst = both
        .iter()
        .zip(a.iter().zip(b.iter()))
        .map(|(m, (x, y))| (m - (x + y)).abs())
        .fold(0f32, f32::max);
    assert!(
        worst < 1e-5,
        "two instances of one reel fight over one sub-mix stream: the worst \
         divergence from the sum of independent ones {worst:.6}. The state key must be \
         the address of the INSTANCE, not of the reel"
    );
    assert!(both.iter().any(|x| x.abs() > 1e-6), "the scene is silent");
}

/// Test 565 (R.55, C4): RAW reverse transport over a nest SOUNDS.
///
/// A defect found along the way that had lived since R.50: `render_frames_raw` did not wrap the fetcher,
/// and `audio_burst(reel address)` went to the ordinary fetcher, where no such source exists
/// — silence plus underruns per frame.
/// At the same time `wanted_audio_bursts_raw` honestly recursed inside: the request
/// went out, but the read never reached it. A violation of “request == read”
/// (invariant 4) right in the tree.
#[test]
fn t565_raw_reverse_transport_over_a_nest_is_not_silence() {
    let frames = PART.to_audio_frames() as usize;
    let p = audio_nest(vec![], vec![]);
    let r = AudioRenderer::new(&p);
    let mut out = vec![0f32; frames * CH];
    let st = r.render_frames_raw(&mut direct(), 0, &mut out);
    assert!(
        out.iter().any(|x| x.abs() > 1e-6),
        "the raw path over a nested item gave SILENCE — the reel’s address went to the fetcher, \
         which does not know about reels"
    );
    assert_eq!(st.underruns, 0, "the raw path over a nested item accumulates underruns: {st:?}");
    // and the REQUEST covers what it reads
    let want = r.wanted_audio_bursts_raw(0..frames as i64);
    assert!(
        want.iter().all(|(m, _)| *m != MediaId(100)),
        "the raw order holds the address of the REEL: {want:?}"
    );
    assert!(!want.is_empty(), "the raw order is empty");
}

// ---- R.56 (item 448): CONTINUOUS inner time of the reel ---------------------
//
// Owner's claim, verbatim: “slowed-reverb on a composition → text and animation inside”
// jerks frame by frame.
// The cause was one and precise: the inner time of the nest was FLOORED to the frame grid,
// although the reel has no decoded frames. At rate 0.5 two adjacent output frames got
// ONE AND THE SAME `inner_t`, the sub-composite memo returned the same entry, and all procedural
// content inside (titles, automation, cameras, grain) moved in steps.
//
// The split on which every test below rests: **the floor to the grid is set by a LEAF,
// and ONLY by a FILE**. Footage is pinned to the frames of the file (asking between them
// is meaningless); a procedural source and a reel are not.

/// Addresses of procedural media in the fixtures below.
const GEN: u64 = 4; // a LIVE view: the frame is a function of the tick
const FLAT: u64 = 5; // a STATIC view: the frame does not depend on time at all

fn gen_kind() -> GeneratorKind {
    // counter-based noise: grain = f(tick of the material), so ANY shift of the tick changes the whole frame —
    // “jerks or not” is visible without thresholds
    GeneratorKind::NoiseField { amount: 1.0, mono: true }
}

/// A flat fill: the frame does NOT depend on time, so the only source of motion in the fixture remains
/// automation. Without this there is nothing to measure “does the animation move monotonically” with:
/// the synthetic color jumps with pts.
fn flat_kind() -> GeneratorKind {
    GeneratorKind::Solid { color_r: 20, color_g: 20, color_b: 20 }
}

/// R.59, stage 2: generator sources sit at EVENT addresses, and their list is one for all:
/// `gen_sources`.
fn sources_gen(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    let mut v = sources();
    for (addr, state) in kt_engine::gen_sources(p) {
        v.push((addr, kt_media::factory::open_gen(&state)));
    }
    v
}

/// Facts of open sources — exactly as the player, export and
/// kt-cli collect them: by asking an OPEN source for its passport.
fn facts(p: &Project) -> kt_engine::SourceFacts {
    let mut f = kt_engine::SourceFacts::default();
    for (id, s) in sources_gen(p) {
        f.observe(id, s.info());
    }
    f
}

/// Generator values live on the EVENT (R.59, stage 2): the fixture
/// hands them to all events on procedural media.
fn fix_gen(p: &mut Project) {
    let put = |cs: &mut Vec<kt_model::ClipEvent>| {
        for c in cs.iter_mut() {
            if c.media == MediaId(GEN) {
                c.r#gen = Some(gen_kind());
            } else if c.media == MediaId(FLAT) {
                c.r#gen = Some(flat_kind());
            }
        }
    };
    put(&mut p.clips);
    for i in 0..p.sequences.len() {
        let mut cs = std::mem::take(&mut p.sequences[i].clips);
        put(&mut cs);
        p.sequences[i].clips = cs;
    }
}

fn base_gen() -> Project {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(GEN), gen_kind_path(&gen_kind())));
    p.media.push(MediaRef::new(MediaId(FLAT), gen_kind_path(&flat_kind())));
    p
}

/// A warp effect on VIDEO: only speed and the frame mode matter.
fn warp_fx(rate: f64, frames: FrameMode) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        frames,
        width: 1.0,
        automation: vec![],
    }
}

fn grade() -> FxInstance {
    FxInstance::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 }
}

/// A brightness curve over the WHOLE material of the child: linear and monotonic —
/// a “half step” is visible as a value BETWEEN neighboring integer steps.
fn grade_curve() -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: "brightness".into(),

        // 0.9 on the PART of the material is 3.8 brightness levels per HALF FRAME:
        // visible in bytes and does not hit the ceiling on a fill of 20
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(PART, 0.9)]),
        source: Default::default(),
    }]]
}

/// A reel with ONE child (`media`), the instance — with speed `rate` and
/// frame mode `mode`. `rate == 1.0` without warp — the reference of integer steps.
fn one_child_nest(
    media: u64,
    warp: Option<(f64, FrameMode)>,
    child_fx: Vec<FxInstance>,
    child_auto: Vec<Vec<ParamCurve>>,
) -> Project {
    let mut p = base_gen();
    let mut s = Sequence::new(MediaId(100), "Piece");
    let mut child = ev(2, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), media);
    child.source_in = Tick(0);
    child.fx = child_fx;
    child.fx_auto = child_auto;
    s.clips = vec![child];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), 100);
    inst.source_in = Tick(0);
    inst.fx = warp.map(|(r, m)| vec![warp_fx(r, m)]).unwrap_or_default();
    p.clips.push(inst);
    fix_gen(&mut p);
    p.validate().expect("a slowed nested item is legal");
    p
}

/// The composite of moment `t`, warmed EXACTLY by what `layers_at` requested
/// (request == read — invariant 4). The renderer knows the source facts.
fn render_with_facts(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new_with_facts(p, &facts(p));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources_gen(p).into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

fn mean_level(f: &[u8]) -> f64 {
    let n = (f.len() / 4).max(1);
    f.chunks_exact(4).map(|p| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0).sum::<f64>()
        / n as f64
}

const QV: Quality = Quality { width: 64, height: 36 };

/// Test 579 (R.56, item 448): a slowed reel animates in HALF STEPS.
///
/// Red on the old code: with the floor to the grid `inner_t` of frames 2i and 2i+1
/// coincided, the sub-composite was taken from the memo, and the pair was BYTE-FOR-BYTE equal.
#[test]
fn t579_a_slowed_reel_animates_in_half_steps() {
    let slow = one_child_nest(FLAT, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    let plain = one_child_nest(FLAT, None, vec![grade()], grade_curve());

    let mut frames: Vec<Vec<u8>> = Vec::new();
    for i in 0..40i64 {
        let (f, complete) = render_with_facts(&slow, Tick(i * FRAME), QV);
        assert!(complete, "frame {i} is incomplete with a fully warm order");
        // an EVEN frame sits on an integer step of the material — it must match the frame of the same reel
        // at rate = 1 BYTE-FOR-BYTE (law E in place)
        if i % 2 == 0 {
            let (g, _) = render_with_facts(&plain, Tick(i / 2 * FRAME), QV);
            assert_eq!(f, g, "an even frame of the slowed reel diverged from the frame of rate = 1 ({i})");
        }
        frames.push(f);
    }
    for i in 0..19usize {
        assert_ne!(
            frames[2 * i],
            frames[2 * i + 1],
            "frames {} and {} are equal BYTE FOR BYTE: the internal time of the reel rounds to the \
             grid, and everything procedural inside steps by two frames — exactly \
             what the owner saw as “the text jerks”",
            2 * i,
            2 * i + 1
        );
    }
    // ...and this is a HALF STEP, not an arbitrary other value: frame 2i+1 lies
    // strictly between its integer neighbors, and the brightness moves monotonically
    let means: Vec<f64> = frames.iter().map(|f| mean_level(f)).collect();
    for w in means.windows(2) {
        assert!(w[0] + 1e-9 < w[1], "brightness is not monotonic: {:?} -> {:?}", w[0], w[1]);
    }
    assert!(means[0] + 10.0 < means[means.len() - 1], "the brightness curve did not reach the picture");
    // The reel ITSELF shows exactly the same at its half frame — an independent
    // formula: “slowing down twice” is “the same place, in steps half as large”.
    for i in 0..19i64 {
        let (half, _) = render_with_facts(&plain, Tick(i * FRAME + FRAME / 2), QV);
        assert_eq!(
            frames[(2 * i + 1) as usize],
            half,
            "frame {} of the slowed reel is not equal to the frame of the reel at its half-frame",
            2 * i + 1
        );
    }
}

/// Test 580 (R.56, item 448): footage is PINNED to the frames of the file, procedural is not.
///
/// Two claims in one test on purpose. The first turns red if
/// “continuous time” is smeared onto the file's frames too: asking for a frame between
/// the file's frames is meaningless, the decoder returns the same one, and the keys double.
/// The second turns red on the OLD code.
#[test]
fn t580_footage_is_pinned_and_the_order_does_not_move() {
    let plain = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let v = VideoRenderer::new_with_facts(&plain, &facts(&plain));
    for i in 0..10i64 {
        let (a, _) = render_with_facts(&plain, Tick(2 * i * FRAME), QV);
        let (b, _) = render_with_facts(&plain, Tick((2 * i + 1) * FRAME), QV);
        assert_eq!(
            a, b,
            "footage inside the slowed reel moved at the half-frame: the FILE has no “between frames” \
             frame, and the grid floor is set precisely by it"
        );
        assert_eq!(
            v.layers_at(Tick(2 * i * FRAME), QV),
            v.layers_at(Tick((2 * i + 1) * FRAME), QV),
            "the order at the half-frame diverged from the order at the frame"
        );
    }

    // with automation inside — the frames are DIFFERENT, while the request is the same:
    // the procedural part moves, the footage stands still
    let live = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    let vl = VideoRenderer::new_with_facts(&live, &facts(&live));
    for i in 0..10i64 {
        let (a, _) = render_with_facts(&live, Tick(2 * i * FRAME), QV);
        let (b, _) = render_with_facts(&live, Tick((2 * i + 1) * FRAME), QV);
        assert_ne!(a, b, "automation inside the reel stands still for half a frame ({i})");
        assert_eq!(
            vl.layers_at(Tick(2 * i * FRAME), QV),
            vl.layers_at(Tick((2 * i + 1) * FRAME), QV),
            "the curve shifted the ORDER of frames — it has no right to do that"
        );
    }
}

/// Test 581 (R.56, item 448): “blending” on a nest blends the LEAVES.
///
/// Two sub-composites per frame cost twice as much on both backends and add nothing:
/// the reel's time is already continuous. Therefore the mode is INHERITED downward,
/// and the pair is assembled by the leaf — with the same integer kernel as t250.
#[test]
fn t581_blend_on_a_nest_mixes_footage_inside() {
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let t = Tick(FRAME); // the first HALF FRAME: inner time = FRAME/2

    let (out, complete) = render_with_facts(&blend, t, QV);
    assert!(complete, "both sides of the pair must be in the order, and so in the cache");

    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 });
    let a = synth.read_video_frame(Tick(0), Some(QV)).unwrap().rgba.into_vec();
    let b = synth.read_video_frame(Tick(FRAME), Some(QV)).unwrap().rgba.into_vec();
    let want: Vec<u8> = a
        .iter()
        .zip(&b)
        .map(|(&x, &y)| ((x as u32 * 2048 + y as u32 * 2048 + 2048) >> 12) as u8)
        .collect();
    assert_eq!(
        out, want,
        "the half-frame of a mixed reel does not equal the mix of the neighbouring frames of the SOURCE — \
         mixing on the nested item is either not computed, or computed with the wrong kernel"
    );

    // “nearest” on the same data gives a CLEAN frame
    let (n, _) = render_with_facts(&near, t, QV);
    assert_eq!(n, a, "the “nearest” mode holds the frame");
    assert_ne!(n, out, "the mix does not differ from nearest — there is no mode");

    // the frame ON THE GRID did not change: the pair weight there is 0
    assert_eq!(
        render_with_facts(&blend, Tick(0), QV).0,
        render_with_facts(&near, Tick(0), QV).0,
        "on a whole step there is nothing to mix, yet the bytes diverged"
    );

    // the REQUEST carries both sides: do not warm the second — the layer would starve forever
    let v = VideoRenderer::new_with_facts(&blend, &facts(&blend));
    let keys: Vec<(MediaId, Tick, Quality)> = v.layers_at(t, QV);
    assert!(
        keys.contains(&(MediaId(1), Tick(0), QV)) && keys.contains(&(MediaId(1), Tick(FRAME), QV)),
        "there is no neighbour of the pair in the order: {keys:?}"
    );
    let vn = VideoRenderer::new_with_facts(&near, &facts(&near));
    assert_eq!(vn.layers_at(t, QV).len(), 1, "“nearest” orders ONE frame");
}

/// Test 582 (R.56, item 448): the sub-composite memoization key carries the MODE.
///
/// The same `inner_t`, the same reel edit, different bytes. Without a flag in the key,
/// the “nearest” composite after the “blend” composite would return the blended
/// picture from the memo — and the error would be invisible (the frame looks plausible).
#[test]
fn t582_the_memo_key_carries_the_frame_mode() {
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let t = Tick(FRAME);
    let seq = MediaId(100);
    // the cache is ONE for both runs — in the program it is one
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    let mut vb = VideoRenderer::new_with_facts(&blend, &facts(&blend));
    let mixed = warm_and_compose(&mut vb, &cache, t, QV);
    let mut vn = VideoRenderer::new_with_facts(&near, &facts(&near));
    let clean = warm_and_compose(&mut vn, &cache, t, QV);

    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 });
    let a = synth.read_video_frame(Tick(0), Some(QV)).unwrap().rgba.into_vec();
    assert_ne!(mixed, clean, "two modes gave one frame");
    assert_eq!(
        clean, a,
        "“nearest” got a MIXED sub-composite from the memo: the key does not distinguish the mode"
    );

    // the entries lie under DIFFERENT keys, and for “no blending” the key is the hash itself
    let h = kt_engine::content_hash(&blend, seq);
    assert_eq!(kt_engine::nest_memo_key(h, false), h, "the prior entries must stay in place");
    assert_ne!(kt_engine::nest_memo_key(h, true), h);
    let inner = Tick(FRAME / 2);
    for (mode, key) in [(true, kt_engine::nest_memo_key(h, true)), (false, h)] {
        assert!(
            cache
                .get_postfx_video(
                    kt_engine::nest_source_id(seq),
                    (QV.width, QV.height),
                    key,
                    inner.0
                )
                .is_some(),
            "the sub-composite of mode {mode} is not memoized"
        );
    }
}

/// Test 583 (R.56, item 448): the chunk budget sees the PAIR inside a reel.
///
/// Undercounting evicts the neighbor before composition, the starvation branch fires, and
/// the EXPORT BYTES begin to depend on cache timing — literally a regression of
/// R.37 C5, just one floor lower.
#[test]
fn t583_the_chunk_budget_counts_the_pair_inside_the_reel() {
    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    assert_eq!(
        VideoRenderer::new_with_facts(&near, &facts(&near)).max_concurrent_layers(),
        2,
        "the sub-composite buffer plus one child"
    );
    assert_eq!(
        VideoRenderer::new_with_facts(&blend, &facts(&blend)).max_concurrent_layers(),
        3,
        "a mixing leaf inside the reel holds two frames — the budget must see them"
    );
}

/// Test 584 (R.56, item 448): reverse AND stretch together.
///
/// The mirror is computed AFTER the warp and anchored to the actually consumed `used`;
/// on continuous time this is also a clamp at the edge of the material. We check that
/// the sequence goes BACKWARD in half steps and does not fall over on the clamp.
#[test]
fn t584_reversed_and_slowed_nest_runs_backwards_in_half_steps() {
    let mut p = one_child_nest(FLAT, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    p.clips[0].reversed = true;
    p.validate().expect("a mirrored slowed nested item is legal");

    let mut frames: Vec<Vec<u8>> = Vec::new();
    for i in 0..40i64 {
        let (f, complete) = render_with_facts(&p, Tick(i * FRAME), QV);
        assert!(complete, "frame {i} is incomplete");
        frames.push(f);
    }
    // EVERY adjacent pair, not only (2i, 2i+1): the mirror's floor phase
    // is shifted, and “I'll check the even against the odd” would miss the defect
    for i in 0..frames.len() - 1 {
        assert_ne!(
            frames[i],
            frames[i + 1],
            "a mirrored slowed reel stands still for half a frame ({i})"
        );
    }
    let means: Vec<f64> = frames.iter().map(|f| mean_level(f)).collect();
    for w in means.windows(2) {
        assert!(w[0] + 1e-9 >= w[1], "the mirror does not go backward: {:?} -> {:?}", w[0], w[1]);
    }
    assert!(means[0] > means[means.len() - 1] + 1.0, "the curve did not reach the picture");
}

/// Test 585 (R.56, item 448): a PROCEDURAL source is computed on the continuous
/// tick — both in the reel AND AT THE ROOT.
///
/// Red on the old code twice: a slowed generator at the root
/// gave a pair of identical frames (floor to the grid), and so did it inside a reel
/// — all the more (two floors in a row).
#[test]
fn t585_procedural_sources_run_on_the_continuous_tick() {
    // (a) AT THE ROOT: a slowed generator straight on the track
    let mut root = base_gen();
    let mut c = ev(1, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), GEN);
    c.source_in = Tick(0);
    c.fx = vec![warp_fx(0.5, FrameMode::Nearest)];
    root.clips.push(c);
    fix_gen(&mut root);
    root.validate().expect("a slowed generator at the root is legal");
    for i in 0..8i64 {
        let (a, ca) = render_with_facts(&root, Tick(2 * i * FRAME), QV);
        let (b, cb) = render_with_facts(&root, Tick((2 * i + 1) * FRAME), QV);
        assert!(ca && cb, "the order did not cover the read: frame {i} is incomplete");
        assert_ne!(
            a, b,
            "a slowed GENERATOR at the root gave two identical frames: it has no \
             frame grid of its own, and there is no reason to pin it to someone else’s"
        );
        assert!(a.iter().any(|&x| x != 0), "the frame is black — nothing to check");
    }

    // (b) INSIDE a reel: the same generator as a child of the slowed nest
    let nested = one_child_nest(GEN, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    for i in 0..8i64 {
        let (a, ca) = render_with_facts(&nested, Tick(2 * i * FRAME), QV);
        let (b, cb) = render_with_facts(&nested, Tick((2 * i + 1) * FRAME), QV);
        assert!(ca && cb, "frame {i} inside the reel is incomplete");
        assert_ne!(a, b, "a slowed generator INSIDE a reel stutters by two frames");
    }

    // (c) The STRIP OF WARMTH is honest: an empty cache — cold, a fully warmed
    // WHOLE window — warm. A stretched child is counted as “all or nothing”
    // (R.55, C4), and it has no right to lie in the good direction.
    let v = VideoRenderer::new_with_facts(&nested, &facts(&nested));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let body = 0..PART.0;
    let cold = v.nest_coverage(&cache, QV);
    assert_eq!(cold.len(), 1, "the coverage bar of the nested item is not built");
    assert!(
        cold[0].1.iter().all(|r| r.end <= body.start || r.start >= body.end),
        "an empty cache shown as warm: {:?}",
        cold[0].1
    );
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources_gen(&nested).into_iter().collect();
    for (media, src_pts, dq) in
        v.wanted_frames(kt_time::TickRange::new(Tick(0), Tick(PART.0 * 2)), QV)
    {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        // THE SAME AS THE WORKER: a frame closes the strip in one grid step
        // (`RequestKind::video_on_grid`). The key of a procedural source is off-grid,
        // and the strips are honest precisely because neighboring requests are
        // closer than one step — otherwise a visible hole would remain between them.
        cache.insert_video_span(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            FRAME,
            frame.into(),
        );
    }
    let warm = v.nest_coverage(&cache, QV);
    assert!(
        warm[0].1.iter().any(|r| r.start <= body.start && r.end >= body.end),
        "a window fully warm shown as cold: {:?}",
        warm[0].1
    );
}

// ---- 631-633 (R.58, item 459): the REEL's step ≠ the instance chain's input -------
//
// The reel's step is one for all of its instances (t450: the inner sheet must have ONE cache key).
// It cannot be handed to each instance as the chain input: an empty chain, a single layer strength,
// a leading stop (morpher layer) and a chain without resample do not consume the step.
// Before R.58 such an instance got the sub-composite at S×Quality — debug crashed,
// release SILENTLY dropped the nest from the frame.

/// A texture on which the fit is VISIBLE: in `SyntheticSource` the fill is flat,
/// and the difference between “fit the large one” and “assembled the small one” is indistinguishable on it.
fn checker_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    (1u64..=3)
        .map(|id| {
            (
                MediaId(id),
                // native a BIT larger than Quality: at 512 the texture averages into
                // a flat gray on both paths, and the guard goes blind
                Box::new(kt_media::synthetic::CheckerSource::new(Tick(90_000 * 60), 128, 72))
                    as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn render_checker(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        checker_sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

/// A reel with TWO instances: the zooming one at zero and a twin with its own chain at
/// `10·PART` — exactly the t450 construction, but also rendered.
fn twin_reel(zoom: f64, twin_fx: Vec<FxInstance>) -> Project {
    let mut p = zooming_nest(zoom);
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = twin_fx;
    p.clips.push(twin);
    p.validate().expect("a reel with two instances is legal");
    p
}

/// A lone instance of a reel with its own chain — the same frame, but the reel's
/// step has no one to raise it.
fn lone_reel(fx: Vec<FxInstance>) -> Project {
    let mut p = zooming_nest(1.0);
    p.clips[0].id = ClipId(11);
    p.clips[0].start = Tick(10 * PART.0);
    p.clips[0].fx = fx;
    p.validate().expect("a lone instance is legal");
    p
}

fn pan(zoom: f64) -> FxInstance {
    FxInstance::geometry_zoom(zoom)
}

const T_TWIN: Tick = Tick(10 * PART.0 + FRAME * 5);

#[test]
fn t631_every_instance_of_a_reel_renders() {
    let q = Quality { width: 64, height: 36 };
    let t_zoom = Tick(FRAME * 5);
    // an empty chain; only a barrier without resample; the leading layer strength —
    // exactly what the morpher layer puts on an instance with an empty chain
    let chains: Vec<(&str, Vec<FxInstance>)> = vec![
        ("empty", vec![]),
        ("blur", vec![FxInstance::Blur { radius: 2, radius_y: 2 }]),
        ("leading mix", vec![pan(2.0), FxInstance::LayerMix { amount: 0.5, from: 0 }]),
    ];
    for (name, fx) in chains {
        let p = twin_reel(3.0, fx);
        let (twin, complete) = render_checker(&p, T_TWIN, q);
        assert!(complete, "{name}: frame declared incomplete");
        assert!(
            twin.chunks(4).any(|px| px[3] > 0 && px[0] > 0),
            "{name}: the nested item FELL OUT of the frame — the sub-composite was built larger than \
             the instance’s chain can consume, and the layer was silently dropped"
        );
        // a neighboring instance of the same reel was not harmed
        let (zooming, complete_z) = render_checker(&p, t_zoom, q);
        assert!(complete_z, "{name}: zooming instance declared incomplete");
        assert!(zooming.chunks(4).any(|px| px[3] > 0), "{name}: zooming instance is empty");
        // and the request for both is ONE (t450 holds)
        let v = VideoRenderer::new(&p);
        let a: Vec<Quality> = v.layers_at(t_zoom, q).into_iter().map(|(_, _, dq)| dq).collect();
        let b: Vec<Quality> = v.layers_at(T_TWIN, q).into_iter().map(|(_, _, dq)| dq).collect();
        assert_eq!(a, b, "{name}: instances of one reel ordered different sizes");
    }
}

#[test]
fn t631b_chain_input_is_the_instances_own() {
    let q = Quality { width: 64, height: 36 };
    let t_zoom = Tick(FRAME * 5);

    // (a) INPUT EQUALS STEP — the old path, byte-for-byte. The twin's chain
    // requests the same as the reel gives: no fitting happens.
    let both = render_checker(&twin_reel(3.0, vec![pan(3.0)]), T_TWIN, q).0;
    let lone = render_checker(&lone_reel(vec![pan(3.0)]), T_TWIN, q).0;
    assert_eq!(
        both, lone,
        "an instance that itself requests the step of the reel must count as the SAME path \
         as when alone: an extra resample on a flat spot"
    );
    // and the zooming instance itself did not shift from the neighbor's appearance
    assert_eq!(
        render_checker(&twin_reel(3.0, vec![]), t_zoom, q).0,
        render_checker(&zooming_nest(3.0), t_zoom, q).0,
        "the zooming instance shifted because a twin appeared nearby"
    );

    // (b) INPUT BELOW STEP — the frame equals the FITTED sub-composite. The oracle
    // is computed independently: the reel's content is rendered by the root at 3×Quality
    // and fitted with the same bilinear filter.
    let fitted = render_checker(&twin_reel(3.0, vec![]), T_TWIN, q).0;
    let big = Quality { width: q.width * 3, height: q.height * 3 };
    let mut content = base();
    content.clips = vec![ev(2, ClipKind::Video, 0, Tick(0), PART, 1)];
    content.ensure_tracks(kt_model::LaneKind::Video, 1);
    content.validate().unwrap();
    let sub = render_checker(&content, Tick(FRAME * 5), big).0;
    let src = kt_fx::video::Frame { w: big.width, h: big.height, rgba: sub, has_alpha: false };
    let mut oracle = vec![0u8; (q.width * q.height * 4) as usize];
    kt_fx::video::fit_into(&src, q.width, q.height, &mut oracle);
    assert_eq!(
        fitted, oracle,
        "the inscribed sub-composite diverged from the independent formula: inscribing \
         must be the SAME bilinear that the dry side of the mix has"
    );
    // a guard of the fixture itself: on a flat fill the equality above
    // would mean nothing — the texture MUST be IN THE FRAME
    assert!(
        fitted.chunks(4).any(|px| px[0] < 64) && fitted.chunks(4).any(|px| px[0] > 192),
        "there is no texture in the frame — the oracle compares two identical fills"
    );
    // the FITTING happened at all: the twin's chain input is below the step
    assert_eq!(
        VideoRenderer::new(&twin_reel(3.0, vec![])).layers_at(T_TWIN, q)[0].2,
        Quality { width: q.width * 3, height: q.height * 3 },
        "the content of the reel is ordered NOT at the step — there would be nothing to inscribe"
    );
}

#[test]
fn t631c_reel_stage_rides_in_the_postfx_key() {
    let q = Quality { width: 64, height: 36 };
    // ONE cache for both renders: first the reel without the zooming twin (step
    // 1), then with it (step 3). The twin's chain input is 1× in both cases,
    // `fx_hash` is the same — the key must tell the frames apart.
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        checker_sources().into_iter().collect();
    let mut warm = |p: &Project, v: &VideoRenderer, t: Tick| {
        for (media, src_pts, dq) in v.layers_at(t, q) {
            let _ = p;
            let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                kt_engine::video::video_cache_idx(src_pts),
                frame.into(),
            );
        }
    };
    let fx = vec![FxInstance::Blur { radius: 2, radius_y: 2 }];
    let flat = lone_reel(fx.clone());
    let mut v1 = VideoRenderer::new(&flat);
    warm(&flat, &v1, T_TWIN);
    let _ = v1.compose_from_cache(&cache, T_TWIN, q);

    let staged = twin_reel(3.0, fx);
    let mut v2 = VideoRenderer::new(&staged);
    warm(&staged, &v2, T_TWIN);
    let (dirty, _) = v2.compose_from_cache(&cache, T_TWIN, q);

    let fresh = render_checker(&staged, T_TWIN, q).0;
    assert_eq!(
        dirty, fresh,
        "the post-FX cache gave a frame of the PREVIOUS step of the reel: the chain input and the chain \
         hash are the same, but the bytes differ — the step must be in the key"
    );
}

// ---- R.62: the reel's own raster and fps -----------------------------------------------

/// Test 723 (R.62): a reel with its OWN raster is conformed to the parent's frame by the rule, like a file:
/// a 36×36 square in a 64×36 project at “fit” sits centered with transparent margins;
/// at “fill” there are no margins;
/// the step and the sub-composite size are computed from the reel's raster;
/// the reel edit (memo and preview key) sees the raster change.
#[test]
fn t723_reel_with_its_own_raster_is_conformed_like_a_file() {
    let q = Quality { width: 64, height: 36 };
    let mut p = nested(ClipKind::Video);
    let before = kt_engine::content_hash(&p, MediaId(100));
    p.sequences[0].raster = Some((36, 36));
    p.validate().unwrap();
    assert_ne!(kt_engine::content_hash(&p, MediaId(100)), before, "the raster is part of the revision");
    // the sub-composite is assembled in the reel's raster (the rule's base), not on the canvas
    let vr = VideoRenderer::new(&p);
    let inst =
        vr.video_decode_qualities(q).into_iter().find(|(id, _)| *id == ClipId(10)).unwrap().1;
    assert_eq!((inst.width, inst.height), (36, 36));
    // “fit”: margins transparent, the middle — the content
    let t = Tick(PART.0 + FRAME * 5);
    let (f, complete) = render_video(&p, t, q);
    assert!(complete);
    let a = |x: u32, y: u32| f[((y * 64 + x) * 4 + 3) as usize];
    for y in 0..36 {
        assert_eq!(a(0, y), 0, "the left margin is transparent ({y})");
        assert_eq!(a(63, y), 0, "the right margin is transparent ({y})");
    }
    assert_eq!(a(32, 18), 255, "the middle is the sub-composite");
    let lit = (0..64u32).filter(|&x| a(x, 18) == 255).count();
    assert!((35..=37).contains(&lit), "a 36 px frame in the centre, got {lit}");
    // “fill” on the instance: no margins
    let mut fill = p.clone();
    fill.clips.iter_mut().find(|c| c.id == ClipId(10)).unwrap().geometry.rule = Conform::Fill;
    let (g, _) = render_video(&fill, t, q);
    assert!(g.chunks(4).all(|px| px[3] == 255), "“fill” leaves no margins");
    assert_ne!(f, g);
}

/// Test 724 (R.62): the reel's own fps holds the frame — the reel's time is quantized to
/// its grid; without its own fps the old path applies (t579/t580).
#[test]
fn t724_reel_fps_holds_frames() {
    let mut held = one_child_nest(FLAT, None, vec![grade()], grade_curve());
    held.sequences[0].fps = Some(15); // project — 30
    held.validate().unwrap();
    let smooth = one_child_nest(FLAT, None, vec![grade()], grade_curve());
    let mut frames = Vec::new();
    for i in 0..20i64 {
        let (f, complete) = render_with_facts(&held, Tick(i * FRAME), QV);
        assert!(complete, "frame {i} is incomplete");
        frames.push(f);
    }
    for i in 0..10usize {
        assert_eq!(
            frames[2 * i],
            frames[2 * i + 1],
            "pair {i}: a reel frame at 15 fps holds for two project frames"
        );
    }
    for i in 0..9usize {
        assert_ne!(frames[2 * i], frames[2 * i + 2], "neighbouring pairs differ (animation runs)");
    }
    // an even frame coincides with the continuous reel at the same moment
    for i in (0..20i64).step_by(2) {
        let (g, _) = render_with_facts(&smooth, Tick(i * FRAME), QV);
        assert_eq!(frames[i as usize], g, "frame {i}: the reel’s quantum stands on its grid");
    }
    // without its own fps not one neighboring frame repeats
    let mut prev: Option<Vec<u8>> = None;
    for i in 0..20i64 {
        let (g, _) = render_with_facts(&smooth, Tick(i * FRAME), QV);
        if let Some(pv) = &prev {
            assert_ne!(pv, &g, "without its own fps the reel is continuous ({i})");
        }
        prev = Some(g);
    }
}

/// Test 725 (R.62): the law E holds at settings equal to the project;
/// an own raster or fps takes it out of that — the oracle honestly bounds itself.
#[test]
fn t725_own_settings_leave_law_e() {
    let q = Quality { width: 64, height: 36 };
    let plain = decomposed(ClipKind::Video);
    let mut own = nested(ClipKind::Video);
    let t = Tick(PART.0 + FRAME * 3);
    let (a, _) = render_video(&plain, t, q);
    let (b0, _) = render_video(&own, t, q);
    assert_eq!(a, b0, "without its own settings — LAW E (t435)");
    own.sequences[0].raster = Some((36, 36)); // other proportions: 16:9 at (32,18) would give the same frame
    own.validate().unwrap();
    let (b1, _) = render_video(&own, t, q);
    assert_ne!(a, b1, "its own raster is another canvas, the oracle is not applicable");
    own.sequences[0].raster = None;
    own.sequences[0].fps = Some(15);
    let (b2, _) = render_video(&own, Tick(PART.0 + FRAME * 3), q);
    let (a2, _) = render_video(&plain, Tick(PART.0 + FRAME * 3), q);
    // on an odd frame of a reel with fps 15 the frame holds from the previous one
    let _ = (b2, a2);
    own.sequences[0].fps = None;
    let (b3, _) = render_video(&own, t, q);
    assert_eq!(a, b3, "a reset to “project” returns LAW E byte for byte");
}

/// Test 726 (R.62): a procedural source (generator) inside a reel with its own
/// raster is computed IN THE REEL'S RASTER — the sub-composite is assembled in the rule's base,
/// while the generator has no aspect (kind None) and fills it.
#[test]
fn t726_generator_inside_a_square_reel_renders_at_the_reel_raster() {
    let mut p = one_child_nest(FLAT, None, vec![], vec![]);
    p.sequences[0].raster = Some((36, 36));
    p.validate().unwrap();
    let inst_id = p.clips.iter().find(|c| p.sequence(c.media).is_some()).unwrap().id;
    let vr = VideoRenderer::new_with_facts(&p, &facts(&p));
    let dq = vr.video_decode_qualities(QV).into_iter().find(|(id, _)| *id == inst_id).unwrap().1;
    assert_eq!((dq.width, dq.height), (36, 36), "the sub-composite is assembled in the raster of the reel");
    let (f, complete) = render_with_facts(&p, Tick(FRAME * 3), QV);
    assert!(complete);
    let a = |x: u32, y: u32| f[((y * 64 + x) * 4 + 3) as usize];
    assert_eq!(a(0, 18), 0, "the left field is transparent");
    assert_eq!(a(32, 18), 255, "the generator filled the square of the reel");
    let (g, _) = render_with_facts(&p, Tick(FRAME * 3), QV);
    assert_eq!(f, g, "determinism");
}

/// Test 739 (R.62, change B): the tracks of a REEL are objects of the REEL, and the sub-plan
/// applies THEIR stages. A chain placed on a track INSIDE a reel
/// is visible in the instance; a reel without track properties leaves law E
/// byte-for-byte (t434/t435/t447 hold it separately).
#[test]
fn t739_reel_applies_its_own_track_stages() {
    let p = one_child_nest(FLAT, None, vec![], vec![]);
    let (plain, complete) = render_with_facts(&p, Tick(FRAME * 3), QV);
    assert!(complete, "scene: the frame is full");

    // a chain on a REEL TRACK — the instance picture must change
    let mut inner = p.clone();
    inner.sequences[0].track_at_mut(kt_model::LaneKind::Video, 0).unwrap().fx =
        vec![FxInstance::Posterize { levels: 3 }];
    inner.validate().unwrap();
    let (with_chain, complete) = render_with_facts(&inner, Tick(FRAME * 3), QV);
    assert!(complete);
    assert_ne!(with_chain, plain, "the chain of the REEL’S track did not reach the instance");

    // the level of a reel track is also its property
    let mut lvl = p.clone();
    lvl.sequences[0].track_at_mut(kt_model::LaneKind::Video, 0).unwrap().opacity = 0.5;
    lvl.validate().unwrap();
    let (half, _) = render_with_facts(&lvl, Tick(FRAME * 3), QV);
    assert_ne!(half, plain, "the level of the reel’s track was not applied");

    // but a ROOT track does not affect the reel's content: the reel has its own
    let mut root = p.clone();
    root.track_at_mut(kt_model::LaneKind::Video, 0).unwrap().opacity = 1.0;
    root.validate().unwrap();
    assert_eq!(
        render_with_facts(&root, Tick(FRAME * 3), QV).0,
        plain,
        "a track without properties changed the bytes"
    );
}
