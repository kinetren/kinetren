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
    // one cache for every project: the same frame under the same name, the wiring alone differs
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    warm(&VideoRenderer::new(&project(vec![clip(1, 0, vec![], vec![], BlendMode::Normal)])), &cache, t);
    let key = |p: &Project| {
        let vr = VideoRenderer::new(p);
        let plan = vr.frame_plan(&cache, t, kt_media::Quality { width: W, height: H });
        kt_engine::moment_key::moment_key(&plan)
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

/// A frame that names its position: the time of a read shows in its pixels.
fn frame_at(pts: Tick) -> Arc<kt_media::VideoFrame> {
    let f = frame();
    let mut rgba = f.rgba.to_vec();
    let tint = (pts.0 / 3000 % 200) as u8;
    for px in rgba.chunks_exact_mut(4) {
        px[0] = px[0].wrapping_add(tint);
        px[2] = px[2].wrapping_sub(tint);
    }
    Arc::new(kt_media::VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false })
}

fn compose_timed(p: &Project, t: Tick) -> (Vec<u8>, Vec<(MediaId, Tick, kt_media::Quality)>) {
    let vr = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: W, height: H };
    let keys = vr.layers_at(t, q);
    for (media, src_pts, dq) in &keys {
        cache.insert_video(kt_engine::source_id(*media), Some(*dq), kt_engine::video::video_cache_idx(*src_pts), frame_at(*src_pts));
    }
    let (out, complete) = kt_gpu::testkit::compose(&vr, &cache, t, q);
    assert!(complete, "the frame did not compose complete");
    (out, keys)
}

fn slowed() -> FxInstance {
    FxInstance::slowed_reverb(0.5, 0.35, 100.0)
}

/// The arm graph: blur from the Source, Slowed from the Source, Screen-merged.
fn arm_graph() -> (Vec<FxInstance>, Vec<SlotInputs>) {
    (
        vec![FxInstance::blur(6), slowed(), merge(BlendMode::Screen, 1.0)],
        vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, merged(FxIn::Slot(0), FxIn::Prev)],
    )
}

/// Test t3226 (fx-graph-dag D2; capability `video-plan`): the read law — an event that merges an
/// arm holding Slowed (0.5×) over an arm holding a blur renders the picture of two copies of the
/// event stacked, the upper slowed by its line, the lower blurred, byte for byte; the slowed read
/// shows another moment than the trunk.
#[test]
fn t3226_the_read_law() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let (fx, fx_in) = arm_graph();
    let graph = project(vec![clip(1, 0, fx, fx_in, BlendMode::Normal)]);
    let stacked = project(vec![
        clip(1, 0, vec![slowed()], vec![], BlendMode::Screen),
        clip(2, 1, vec![FxInstance::blur(6)], vec![], BlendMode::Normal),
    ]);
    for t in [Tick(SPAN / 4), Tick(SPAN / 2), Tick(SPAN * 3 / 4)] {
        let (a, keys) = compose_timed(&graph, t);
        let (b, _) = compose_timed(&stacked, t);
        let pts: std::collections::BTreeSet<i64> = keys.iter().map(|k| k.1.0).collect();
        assert!(pts.len() >= 2, "at {t:?} the slowed read did not read another moment: {keys:?}");
        let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert_eq!(diff, 0, "at {t:?} the read differs from the stacked slowed layer in {diff} bytes");
    }
}

/// Test t3227 (fx-graph-dag D2): the order sees both reads — `layers_at` lists the trunk's frame
/// and the slowed arm's half-rate frame, the composite misses none (it is complete), and the line
/// without the arm orders one frame.
#[test]
fn t3227_the_order_sees_both_reads() {
    kt_plugins_builtin::install();
    let q = kt_media::Quality { width: W, height: H };
    let t = Tick(SPAN / 2);
    let (fx, fx_in) = arm_graph();
    let graph = VideoRenderer::new(&project(vec![clip(1, 0, fx, fx_in, BlendMode::Normal)]));
    let line = VideoRenderer::new(&project(vec![clip(1, 0, vec![FxInstance::blur(6)], vec![], BlendMode::Normal)]));
    let slow = VideoRenderer::new(&project(vec![clip(1, 0, vec![slowed()], vec![], BlendMode::Normal)]));
    let mut want = line.layers_at(t, q);
    want.extend(slow.layers_at(t, q));
    assert_eq!(graph.layers_at(t, q), want, "the graph's order is not its two reads'");
    assert_eq!(line.layers_at(t, q).len(), 1, "a line orders one frame");
}

/// Test t3205 (fx-graph-dag D2, D3; capability `video-plan`): a graph whose arms hold no time kind
/// reads as its line — a slowed event that branches after it asks the line's frames at every moment;
/// with the zooming lead before the branch the layer keeps the line's decode step; a zoom that stands
/// in an arm (the branch before any geometry) gives step 1.
#[test]
fn t3205_a_graph_without_a_time_kind_in_an_arm_reads_as_the_line() {
    kt_plugins_builtin::install();
    let q = kt_media::Quality { width: W, height: H };
    let natives = std::collections::HashMap::from([(MediaId(1), (W * 4, H * 4))]);
    let build = |p: &Project| VideoRenderer::new_with_natives(p, &natives);
    let zoomed = |fx: Vec<FxInstance>, w: Vec<SlotInputs>| {
        let (mut c, b) = clip(1, 0, fx, w, BlendMode::Normal);
        c.geometry.zoom = 2.0;
        project(vec![(c, b)])
    };
    let from = |j: u32| SlotInputs { main: FxIn::Slot(j), ..Default::default() };
    // Slowed on the trunk, two arms after it
    let graph = build(&zoomed(
        vec![slowed(), FxInstance::blur(6), duotone(), merge(BlendMode::Screen, 1.0)],
        vec![SlotInputs::default(), SlotInputs::default(), from(0), merged(FxIn::Slot(1), FxIn::Prev)],
    ));
    let line = build(&zoomed(vec![slowed(), FxInstance::blur(6)], vec![]));
    for k in 0..30 {
        let t = Tick(k * 3000);
        assert_eq!(graph.layers_at(t, q), line.layers_at(t, q), "at {t:?} the graph asked other frames than its line");
    }
    assert!(line.layers_at(Tick(0), q)[0].2.width > W, "scene: the zooming lead raises the decode step");
    assert!(graph.read_rows(q).is_empty(), "no arm holds a time kind: no second read");
    // the zoom in an arm: the branch stands before any geometry
    let (c, b) = clip(
        1,
        0,
        vec![FxInstance::geometry_zoom(2.0), FxInstance::blur(6), merge(BlendMode::Screen, 1.0)],
        vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, merged(FxIn::Slot(0), FxIn::Prev)],
        BlendMode::Normal,
    );
    let early = build(&project(vec![(c, b)]));
    assert_eq!(early.layers_at(Tick(0), q)[0].2, q, "a zoom in an arm raised the layer's decode step");
}

fn half_mask(x: f64) -> Mask {
    Mask::new(1, "half", MaskShape::Rect { cx: x, cy: 0.5, w: 0.5, h: 1.0, rot: 0.0, round: 0.0 })
}

/// Test t3206 (fx-graph-dag D3; capability `video-plan`): masks in a graph are the stacked layers'
/// masks — a mask on a slot of an arm takes the slot's OWN input as its dry side (not what ran before
/// it in the list), a mask of a read's slot cuts inside the read, and the picture's masks cut before
/// the branch, so every arm gets the cut frame.
#[test]
fn t3206_masks_in_a_graph_are_the_stacked_layers_masks() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let t = Tick(SPAN / 2);
    let src = || SlotInputs { main: FxIn::Source, ..Default::default() };
    let same = |what: &str, lsb: u8, graph: &Project, stacked: &Project| {
        let (a, _) = compose_timed(graph, t);
        let (b, _) = compose_timed(stacked, t);
        let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        let worst = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        assert!(diff == 0 || (worst <= lsb), "{what}: the graph differs from the stacked layers in {diff} bytes, by up to {worst}");
    };
    let masked = |c: (ClipEvent, BlendMode), slot: Option<usize>, x: f64| {
        let (mut c, b) = c;
        c.masks.list_mut(slot).push(half_mask(x));
        (c, b)
    };
    // the duotone arm's slot is masked: outside it shows the source, not the blur before it
    let fx = vec![FxInstance::blur(6), duotone(), merge(BlendMode::Screen, 1.0)];
    let w = vec![SlotInputs::default(), src(), merged(FxIn::Slot(0), FxIn::Prev)];
    let plain = project(vec![clip(1, 0, fx.clone(), w.clone(), BlendMode::Normal)]);
    let graph = project(vec![masked(clip(1, 0, fx, w, BlendMode::Normal), Some(1), 0.25)]);
    assert!(compose_timed(&graph, t).0 != compose_timed(&plain, t).0, "scene: the mask does something");
    same(
        "a masked slot of an arm",
        0,
        &graph,
        &project(vec![
            masked(clip(1, 0, vec![duotone()], vec![], BlendMode::Screen), Some(0), 0.25),
            clip(2, 1, vec![FxInstance::blur(6)], vec![], BlendMode::Normal),
        ]),
    );
    // a masked slot of a READ's line (duotone before Slowed in the arm)
    let fx = vec![FxInstance::blur(6), duotone(), slowed(), merge(BlendMode::Screen, 1.0)];
    let w = vec![SlotInputs::default(), src(), SlotInputs::default(), merged(FxIn::Slot(0), FxIn::Prev)];
    let plain = project(vec![clip(1, 0, fx.clone(), w.clone(), BlendMode::Normal)]);
    let graph = project(vec![masked(clip(1, 0, fx, w, BlendMode::Normal), Some(1), 0.75)]);
    assert!(compose_timed(&graph, t).0 != compose_timed(&plain, t).0, "scene: the read's mask does something");
    same(
        "a masked slot of a read",
        0,
        &graph,
        &project(vec![
            masked(clip(1, 0, vec![duotone(), slowed()], vec![], BlendMode::Screen), Some(0), 0.75),
            clip(2, 1, vec![FxInstance::blur(6)], vec![], BlendMode::Normal),
        ]),
    );
    // the picture's mask: every arm, the read among them
    let (fx, w) = arm_graph();
    let plain = project(vec![clip(1, 0, fx.clone(), w.clone(), BlendMode::Normal)]);
    let graph = project(vec![masked(clip(1, 0, fx, w, BlendMode::Normal), None, 0.25)]);
    assert!(compose_timed(&graph, t).0 != compose_timed(&plain, t).0, "scene: the picture's mask does something");
    same(
        "the picture's mask",
        1,
        &graph,
        &project(vec![
            masked(clip(1, 0, vec![slowed()], vec![], BlendMode::Screen), None, 0.25),
            masked(clip(2, 1, vec![FxInstance::blur(6)], vec![], BlendMode::Normal), None, 0.25),
        ]),
    );
}

/// Test t3243 (fx-graph-dag D3, 2.6): a register used again gives the picture of a register of its own —
/// a chain of five merges one after another (more held values than `MAX_HELD`, never more than that at
/// once), a slot that makes no pass between a load and the next hold, and a frame after a frame on the
/// same engine are the same bytes as the arms computed apart.
#[test]
fn t3243_a_register_used_again_gives_the_same_picture() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let t = Tick(SPAN / 2);
    let src = || SlotInputs { main: FxIn::Source, ..Default::default() };
    // blur | duotone, merge | vignette, merge | … — each arm from the Source
    // (the fourth arm is a neutral geometry: a slot that makes no pass over a loaded Source)
    let slot = |id: &str| if id == "neutral" { FxInstance::geometry_zoom(1.0) } else { FxInstance::plugin(id).unwrap() };
    let arms = ["duotone", "vignette", "posterize", "neutral", "color_grade"];
    let mut fx = vec![FxInstance::blur(6)];
    let mut w = vec![SlotInputs::default()];
    for (n, id) in arms.iter().enumerate() {
        let at = fx.len() as u32;
        fx.push(slot(id));
        w.push(src());
        fx.push(merge(BlendMode::Screen, 0.5));
        w.push(merged(FxIn::Slot(if n == 0 { 0 } else { at - 1 }), FxIn::Prev));
    }
    assert!(kt_model::fx_graph::held_peak(&w, fx.len()) <= kt_model::fx_graph::MAX_HELD, "scene");
    let graph = project(vec![clip(1, 0, fx, w, BlendMode::Normal)]);
    let a = compose(&graph, t);
    // the same arms one merge deep at a time: the running result is the Background of the next
    let mut want = compose(&project(vec![clip(1, 0, vec![FxInstance::blur(6)], vec![], BlendMode::Normal)]), t);
    for id in arms {
        let arm = compose(&project(vec![clip(1, 0, vec![slot(id)], vec![], BlendMode::Normal)]), t);
        for (b, f) in want.chunks_exact_mut(4).zip(arm.chunks_exact(4)) {
            for c in 0..3 {
                let (x, y) = (b[c] as i32, f[c] as i32);
                let screen = x + y - (x * y + 127) / 255;
                b[c] = (x + (screen - x) / 2).clamp(0, 255) as u8;
            }
        }
    }
    let worst = a.iter().zip(&want).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
    assert!(worst <= 4, "the chain of merges is off its arms by {worst}");
    assert!(a == compose(&graph, t), "two compositions differ");
}
