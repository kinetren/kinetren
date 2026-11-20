//! fx-graph-dag v2b (capability `video-plan`): an event's effect GRAPH through the engine — the
//! renderer hands the wiring to the one planner, the Merge is the composite's operation, and the
//! keys see the wiring.

use kt_engine::VideoRenderer;
use kt_model::fx_graph::{FxIn, Matte, MatteFrom, SlotInputs};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;

const W: u32 = 64;
const H: u32 = 36;
const SPAN: i64 = 90_000;

fn frame() -> Arc<kt_media::VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            rgba[i] = (x * 4) as u8;
            rgba[i + 1] = (y * 7) as u8;
            rgba[i + 2] = ((x + y) * 3) as u8;
            rgba[i + 3] = 255;
        }
    }
    Arc::new(kt_media::VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false })
}

fn clip(id: u64, lane: u32, fx: Vec<FxInstance>, fx_in: Vec<SlotInputs>, blend: BlendMode) -> (ClipEvent, BlendMode) {
    (
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
            duration: Tick(SPAN),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx,
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
            fx_in,
        },
        blend,
    )
}

fn project(clips: Vec<(ClipEvent, BlendMode)>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: W,
        height: H,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let lanes = clips.iter().map(|(c, _)| c.lane + 1).max().unwrap_or(1);
    p.ensure_tracks(LaneKind::Video, lanes);
    for (c, blend) in clips {
        let lane = c.lane;
        if let Some(i) = track_index(&p.tracks, LaneKind::Video, lane) {
            p.tracks[i].blend = blend;
        }
        p.clips.push(c);
    }
    p.validate().unwrap();
    p
}

fn warm(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick) {
    let q = kt_media::Quality { width: W, height: H };
    for (media, src_pts, q) in vr.layers_at(t, q) {
        cache.insert_video(kt_engine::source_id(media), Some(q), kt_engine::video::video_cache_idx(src_pts), frame());
    }
}

fn compose(p: &Project, t: Tick) -> Vec<u8> {
    let vr = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    warm(&vr, &cache, t);
    let q = kt_media::Quality { width: W, height: H };
    let (out, complete) = kt_gpu::testkit::compose(&vr, &cache, t, q);
    assert!(complete, "the frame did not compose complete");
    out
}

fn duotone() -> FxInstance {
    FxInstance::plugin("duotone").expect("duotone")
}

fn merge(mode: BlendMode, mix: f64) -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Blend(mode), mix }
}

fn merged(main: FxIn, second: FxIn) -> SlotInputs {
    SlotInputs { main, second: Some(second), matte: None }
}

/// Test t3204 (fx-graph-dag D5; capability `video-plan`): for each of the nine modes, ONE event
/// whose graph merges a duotone arm over a blurred arm renders the picture of TWO copies of the event
/// stacked on adjacent tracks — the lower with the blur, the upper with the duotone and the blend
/// mode — byte for byte on one device.
#[test]
fn t3204_one_event_with_a_merge_equals_two_stacked_layers() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let t = Tick(SPAN / 2);
    let blurred = compose(&project(vec![clip(1, 0, vec![FxInstance::blur(6)], vec![], BlendMode::Normal)]), t);
    for mode in BlendMode::ALL {
        let graph = project(vec![clip(
            1,
            0,
            vec![FxInstance::blur(6), duotone(), merge(mode, 1.0)],
            vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, merged(FxIn::Slot(0), FxIn::Prev)],
            BlendMode::Normal,
        )]);
        // the upper track (lane 0 is the top: "lower = higher") carries the blend mode
        let stacked = project(vec![
            clip(1, 0, vec![duotone()], vec![], mode),
            clip(2, 1, vec![FxInstance::blur(6)], vec![], BlendMode::Normal),
        ]);
        let (a, b) = (compose(&graph, t), compose(&stacked, t));
        let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert_eq!(diff, 0, "{mode:?}: the Merge differs from the two stacked layers in {diff} bytes");
        assert_ne!(a, blurred, "{mode:?}: the Foreground did nothing — the law would hold vacuously");
    }
}

/// Test t3201 (fx-graph-dag D3): the highlights blurred and laid back — a blur merged over the
/// Source by Screen through the Source's luma — composes the same bytes cold and warm, through a
/// fresh renderer, and as the one-frame chain the testkit runs; it differs from both arms alone.
#[test]
fn t3201_highlights_blurred_and_laid_back() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let fx = vec![FxInstance::blur(8), merge(BlendMode::Screen, 1.0)];
    let fx_in = vec![
        SlotInputs::default(),
        SlotInputs { main: FxIn::Source, second: Some(FxIn::Prev), matte: Some(Matte { from: FxIn::Source, by: MatteFrom::Luma }) },
    ];
    let p = project(vec![clip(1, 0, fx.clone(), fx_in.clone(), BlendMode::Normal)]);
    let t = Tick(SPAN / 2);
    let vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    warm(&vr, &cache, t);
    let q = kt_media::Quality { width: W, height: H };
    let (cold, c1) = kt_gpu::testkit::compose(&vr, &cache, t, q);
    let (again, c2) = kt_gpu::testkit::compose(&vr, &cache, t, q);
    assert!(c1 && c2);
    assert_eq!(cold, again, "the warm frame differs from the cold one");
    assert_eq!(compose(&p, t), cold, "a fresh renderer gave other bytes");
    // the engine's path is the planner's: the same graph over the same frame by the testkit
    let f = frame();
    let ctx = kt_fx::video::VideoFxCtx { out_w: W, out_h: H, u: 0.5, burst_index: t.0, cam3d: None, in_scale: 1, lead: None };
    let direct = kt_gpu::testkit::graph(
        &fx,
        &fx_in,
        kt_fx::video::Frame { w: W, h: H, rgba: f.rgba.to_vec(), has_alpha: false },
        &ctx,
    );
    assert_eq!(direct.rgba, cold, "the renderer's graph is not the planner's");
    let plain = compose(&project(vec![clip(1, 0, vec![], vec![], BlendMode::Normal)]), t);
    let blurred = compose(&project(vec![clip(1, 0, vec![FxInstance::blur(8)], vec![], BlendMode::Normal)]), t);
    assert_ne!(cold, plain, "the merge did nothing");
    assert_ne!(cold, blurred, "the matte let the whole blur through");
}

/// Test t3208 (fx-graph-dag D3; capability `video-plan`): the keys see the wiring — the same slots
/// wired as a graph, and wired otherwise, give other plan keys; a line written out as entries is
/// the line's key.
#[test]
fn t3208_the_keys_see_the_wiring() {
    let t = Tick(SPAN / 2);
    let key = |p: &Project| {
        let vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t);
        let plan = vr.frame_plan(&cache, t, kt_media::Quality { width: W, height: H });
        kt_engine::render_cache::durable_hash(&plan)
    };
    kt_plugins_builtin::install();
    let fx = vec![FxInstance::blur(6), duotone(), merge(BlendMode::Screen, 1.0)];
    let line = key(&project(vec![clip(1, 0, fx.clone(), vec![], BlendMode::Normal)]));
    let a = key(&project(vec![clip(
        1,
        0,
        fx.clone(),
        vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, merged(FxIn::Slot(0), FxIn::Prev)],
        BlendMode::Normal,
    )]));
    let b = key(&project(vec![clip(
        1,
        0,
        fx.clone(),
        vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, merged(FxIn::Prev, FxIn::Slot(0))],
        BlendMode::Normal,
    )]));
    assert_ne!(line, a, "a graph has the line's key");
    assert_ne!(a, b, "two wirings of the same slots share a key");
    let mut written = project(vec![clip(1, 0, fx, vec![], BlendMode::Normal)]);
    written.clips[0].fx_in = vec![SlotInputs::default(), SlotInputs { main: FxIn::Slot(0), ..Default::default() }];
    written.normalize();
    assert_eq!(key(&written), line, "a line written out moved the key");
}

/// Test t3202 (fx-graph-dag D3): a matte made from the picture — a blurred arm merged over the
/// Source through the Source's luma: each pixel is the integer mix of the two by the coverage
/// `luma × mix` (`matte.wgsl` then `layer_mix.wgsl`'s coverage branch), at mix 1 and 0.5; the
/// inverted luma gives the complement.
#[test]
fn t3202_a_matte_made_from_the_picture() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let t = Tick(SPAN / 2);
    let src = compose(&project(vec![clip(1, 0, vec![], vec![], BlendMode::Normal)]), t);
    let blur = compose(&project(vec![clip(1, 0, vec![FxInstance::blur(8)], vec![], BlendMode::Normal)]), t);
    for (mix, by) in [(1.0, MatteFrom::Luma), (0.5, MatteFrom::Luma), (1.0, MatteFrom::InvLuma)] {
        let fx = vec![FxInstance::blur(8), merge(BlendMode::Normal, mix)];
        let fx_in = vec![
            SlotInputs::default(),
            SlotInputs { main: FxIn::Source, second: Some(FxIn::Prev), matte: Some(Matte { from: FxIn::Source, by }) },
        ];
        let out = compose(&project(vec![clip(1, 0, fx, fx_in, BlendMode::Normal)]), t);
        let k = (mix * 4096.0f64).round() as u32;
        let mut wrong = 0;
        for (i, px) in out.chunks_exact(4).enumerate() {
            let s = &src[i * 4..i * 4 + 4];
            let b = &blur[i * 4..i * 4 + 4];
            let luma = (54 * s[0] as u32 + 183 * s[1] as u32 + 19 * s[2] as u32 + 128) >> 8;
            let m = if by == MatteFrom::InvLuma { 255 - luma } else { luma };
            let cov = (m * k + 2048) >> 12;
            for c in 0..4 {
                let want = (s[c] as u32 * (255 - cov) + b[c] as u32 * cov + 127) / 255;
                if px[c] as u32 != want {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0, "mix {mix}, {by:?}: {wrong} bytes are not the matte's mix");
    }
}
