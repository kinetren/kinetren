//! shape-masks: the masks in the plan — the picture follows its Geometry node, a slot mask limits
//! an effect, a track mask cuts the track's composite, no masks — the earlier bytes.

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 64, height: 36 };
/// The synthetic frame is painted by the tick: the top event reads tick 3000 (r = 184).
const TOP_IN: i64 = 3_000;

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

fn clip(id: u64, lane: u32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// A top event over a bottom one (r = 184 over r = 0).
fn two_layers() -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:silence"));
    let mut top = clip(1, 0);
    top.source_in = Tick(TOP_IN);
    p.clips = vec![top, clip(2, 1)];
    p.normalize();
    p.validate().unwrap();
    p
}

fn render_with(p: &Project, cache: &kt_cache::BurstCache) -> Vec<u8> {
    let src = || SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    let mut facts = kt_engine::SourceFacts::default();
    facts.observe(MediaId(1), src().info());
    let video = VideoRenderer::new_with_facts(p, &facts);
    let mut s = src();
    for (media, pts, dq) in video.layers_at(Tick(0), Q) {
        let frame = s.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video::video_cache_idx(pts), frame.into());
    }
    kt_gpu::testkit::compose(&video, cache, Tick(0), Q).0
}

fn render(p: &Project) -> Vec<u8> {
    render_with(p, &kt_cache::BurstCache::new(kt_cache::CacheConfig::default()))
}

fn px(f: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * Q.width + x) * 4) as usize;
    [f[i], f[i + 1], f[i + 2], f[i + 3]]
}

fn rect(cx: f64, cy: f64, w: f64, h: f64) -> Mask {
    Mask::new(1, "Mask 1", MaskShape::Rect { cx, cy, w, h, rot: 0.0, round: 0.0 })
}

/// Test 1270: the picture mask cuts the event and follows its Geometry node.
#[test]
fn t1270_the_picture_mask_cuts_the_event_and_follows_its_geometry() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let mut p = two_layers();
    p.clips[0].masks.picture.push(rect(0.25, 0.5, 0.5, 1.2));
    let f = render(&p);
    assert_eq!(px(&f, 8, 18)[0], 184, "inside the mask — the event");
    assert_eq!(px(&f, 56, 18)[0], 0, "outside — the track below");

    // the event moved right by a quarter of the canvas: the region moved with it
    let mut q = p.clone();
    q.clips[0].geometry.pos_x = 0.25;
    let f = render(&q);
    assert_eq!(px(&f, 8, 18)[0], 0, "left of the moved event");
    assert_eq!(px(&f, 24, 18)[0], 184, "the masked half moved with the event");
    assert_eq!(px(&f, 40, 18)[0], 184);
    assert_eq!(px(&f, 52, 18)[0], 0, "the unmasked half of the moved event");

    // inverted — the halves swap
    let mut r = p.clone();
    r.clips[0].masks.picture[0].invert = true;
    let f = render(&r);
    assert_eq!(px(&f, 8, 18)[0], 0);
    assert_eq!(px(&f, 56, 18)[0], 184);
}

/// Test 1271: a slot mask limits the effect to its region, before and after the Geometry node.
#[test]
fn t1271_a_slot_mask_limits_an_effect_to_the_region() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let mut base = two_layers();
    base.clips.truncate(1);
    let plain = render(&base);
    let mut fxp = base.clone();
    fxp.clips[0].fx = vec![FxInstance::plugin("duotone").unwrap()];
    let wet = render(&fxp);
    let (dry_c, wet_c) = (px(&plain, 32, 18), px(&wet, 32, 18));
    assert_ne!(dry_c, wet_c, "the effect must change the colour for this test to mean anything");

    let mut m = fxp.clone();
    m.clips[0].masks.slots = vec![vec![Mask::new(1, "e", MaskShape::Ellipse { cx: 0.5, cy: 0.5, w: 0.5, h: 0.5, rot: 0.0 })]];
    let f = render(&m);
    assert_eq!(px(&f, 32, 18), wet_c, "inside the ellipse — the effect");
    assert_eq!(px(&f, 2, 2), dry_c, "outside — the event without the effect");
    assert!(m.clips[0].masks.picture.is_empty());

    // 50 %: the half-way mix, exactly
    let mut h = m.clone();
    h.clips[0].masks.slots[0][0].opacity = 0.5;
    let f = render(&h);
    let mix = |d: u8, w: u8| ((d as u32 * 127 + w as u32 * 128 + 127) / 255) as u8;
    let got = px(&f, 32, 18);
    for c in 0..3 {
        assert_eq!(got[c], mix(dry_c[c], wet_c[c]), "channel {c}");
    }

    // zoomed ×2: the region scales with the content, before and after the node
    for pos in [0usize, 1] {
        let mut z = m.clone();
        z.clips[0].geometry.zoom = 2.0;
        z.clips[0].geometry_pos = pos;
        let f = render(&z);
        assert_eq!(px(&f, 8, 18), wet_c, "geometry_pos {pos}: the zoomed region did not grow");
        let f0 = render(&m);
        assert_eq!(px(&f0, 8, 18), dry_c, "unzoomed, x = 8 is outside");
    }
}

/// Test 1272: a track mask cuts the track's composite, the level multiplies only inside.
#[test]
fn t1272_a_track_mask_cuts_the_tracks_composite() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let p = two_layers();
    let plain = render(&p);
    let mut q = p.clone();
    let vt = q.tracks.iter().position(|t| t.kind == LaneKind::Video).unwrap();
    assert!(!q.tracks[vt].needs_video_stage());
    q.tracks[vt].masks.push(rect(0.25, 0.5, 0.5, 1.2));
    assert!(q.tracks[vt].needs_video_stage(), "masks make a track stage");
    let f = render(&q);
    assert_eq!(px(&f, 8, 18)[0], 184, "inside — the track's composite");
    assert_eq!(px(&plain, 56, 18)[0], 184, "without the mask the top track covers everything");
    assert_eq!(px(&f, 56, 18), [0, 0, 0, 255], "outside — the lower track, untouched");
    q.tracks[vt].opacity = 0.5;
    let f = render(&q);
    let v = px(&f, 8, 18)[0] as i32;
    assert!((v - 92).abs() <= 1, "the level inside: {v}");
    assert_eq!(px(&f, 56, 18)[0], 0, "outside nothing of the track");
}

/// Test 1273: without masks the earlier bytes; a mask edit is never a stale frame; a repeat
/// comes from the cache.
#[test]
fn t1273_no_masks_earlier_bytes_and_an_edit_is_not_stale() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let p = two_layers();
    let plain = render(&p);
    let mut q = p.clone();
    q.clips[0].masks.picture.push(rect(0.5, 0.5, 0.5, 0.5));
    q.clips[0].masks.picture.clear();
    assert_eq!(render(&q), plain, "an emptied mask list moved the bytes");

    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut m = p.clone();
    let mut mask = rect(0.5, 0.5, 0.5, 0.5);
    mask.feather = 4.0;
    m.clips[0].masks.picture.push(mask);
    let a = render_with(&m, &cache);
    let hits = cache.stats().video_hits;
    let b = render_with(&m, &cache);
    assert_eq!(a, b);
    assert!(cache.stats().video_hits > hits, "the repeat did not come from the post-FX cache");
    m.clips[0].masks.picture[0].feather = 12.0;
    let c = render_with(&m, &cache);
    assert_ne!(c, a, "a feather edit returned the stale frame");
    assert_eq!(c, render(&m), "the edited frame differs from a cold one");
}
