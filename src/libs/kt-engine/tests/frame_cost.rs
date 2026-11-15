//! Tests 2660–2679 (frame-work-by-edit): THE WORK OF A FRAME GROWS WITH THE FRAME, THE WORK OF AN
//! EDIT WITH THE EDIT — not with the project.
//!
//! Every per-frame question (the plan, the order, the coverage of a reel, a stream stage) and every
//! per-edit lookup (a reel by id, a staircase transition's third event) once scanned the whole
//! project. Each scene here is GENERATED at ×1, ×4 and ×16 events that the frame or the edit does
//! not touch, and what is read is a COUNT (`kt_time::cost`), never a clock (t1671): the count at
//! ×16 must equal the count at ×1.

use kt_engine::VideoRenderer;
use kt_media::Quality;
use kt_model::*;
use kt_time::{Tick, TickRange};
use kt_time::cost::{Cost, Costs};

const SEC: i64 = 90_000;
const Q: Quality = Quality { width: 64, height: 36 };

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p
}

fn ev(id: u64, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
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
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
    }
}

fn curve(param: &str, a: f64, b: f64) -> ParamCurve {
    ParamCurve { param: param.into(), keys: vec![AutoKey::at(Tick(0), a), AutoKey::at(Tick(4 * SEC), b)], source: Default::default() }
}

/// The frame's own scene in the first two seconds — a dissolve with an animated wipe, an animated
/// layer, an animated mask, a camera with a curve — and `far` one-second events after 100 s on
/// two lanes, with crossfades.
fn scene(far: u64) -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, 3);
    p.ensure_tracks(LaneKind::Camera, 1);
    let mut a = ev(1, 0, 0, SEC, 1);
    a.fx = vec![FxInstance::builtin("vignette")];
    a.fx_auto = vec![vec![curve("strength", 0.1, 0.9)]];
    let mut b = ev(2, 0, SEC, SEC, 1);
    b.masks.picture = vec![{
        let mut m = Mask::new(1, "m", MaskShape::Ellipse { cx: 0.5, cy: 0.5, w: 0.5, h: 0.5, rot: 0.0 });
        m.auto = vec![curve("w", 0.2, 0.8)];
        m
    }];
    let mut c = ev(3, 1, 0, 2 * SEC, 1);
    c.fx = vec![FxInstance::builtin("pan3d")];
    p.clips.extend([a, b, c]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(SEC / 5),
        out_offset: Tick(SEC / 5),
        kind: TransitionKind::plugin_with("wipe", &[0.0, 0.0, 0.0]).unwrap(),
        audio: Default::default(),
        auto: vec![curve("angle", 0.0, 90.0)],
        invert: false,
    });
    p.cameras.push(CameraEvent {
        ease: true,
        id: CameraId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(10 * SEC),
        name: String::new(),
        cam: CameraParams::default(),
        keyframes: vec![],
        auto: vec![curve("rot_y", 0.0, 30.0)],
    });
    for i in 0..far {
        for lane in [0u32, 2] {
            let id = 1000 + 4 * i + lane as u64;
            p.clips.push(ev(id, lane, (100 + i as i64) * SEC, SEC, 1));
        }
        if i > 0 {
            p.transitions.push(TransitionEvent {
                id: TransitionId(1000 + i),
                a: Some(ClipId(1000 + 4 * (i - 1))),
                b: Some(ClipId(1000 + 4 * i)),
                in_offset: Tick(SEC / 10),
                out_offset: Tick(SEC / 10),
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
    }
    p.validate().expect("the scene is valid");
    p
}

fn count<R>(f: impl FnOnce() -> R) -> Costs {
    kt_time::cost::meter(f).1
}

fn row(c: &Costs, r: Cost) -> u64 {
    c[r as usize]
}

fn cache() -> kt_cache::BurstCache {
    kt_cache::BurstCache::new(kt_cache::CacheConfig::default())
}

/// The per-frame work of `ask` at ×1, ×4, ×16 far events: equal.
fn holds_flat(what: &str, ask: impl Fn(&VideoRenderer, &kt_cache::BurstCache) -> Costs) {
    kt_plugins_builtin::install();
    let cache = cache();
    let counts: Vec<Costs> = [1u64, 4, 16]
        .into_iter()
        .map(|n| {
            let v = VideoRenderer::new(&scene(n * 20));
            ask(&v, &cache)
        })
        .collect();
    assert!(row(&counts[0], Cost::FrameClip) > 0, "{what}: the scene's frame looked at no event");
    for c in &counts[1..] {
        assert_eq!(c, &counts[0], "{what}: a frame's work grew with events it does not show");
    }
}

/// t2660: a frame's plan looks at the events active at it, whatever the edit's length.
#[test]
fn t2660_a_frame_plan_costs_its_events() {
    holds_flat("frame_plan", |v, cache| {
        count(|| {
            for f in 0..50 {
                drop(v.frame_plan(cache, Tick(f * SEC / 25), Q));
            }
        })
    });
}

/// t2661: the order (`layers_at`, the prefetch's `wanted_frames`) costs the window's frames.
#[test]
fn t2661_the_order_costs_the_window() {
    holds_flat("wanted_frames", |v, _| {
        count(|| drop(v.wanted_frames(kt_time::TickRange::new(Tick(0), Tick(2 * SEC)), Q)))
    });
}

/// t2662: the time index hands exactly the events the scan found, in its order, at every tick a
/// boundary or a handle can sit on.
#[test]
fn t2662_the_index_is_the_scan() {
    kt_plugins_builtin::install();
    let mut p = scene(8);
    // overlaps on one lane and a long event under everything
    p.clips.push(ev(90, 1, SEC / 2, 150 * SEC, 1));
    p.clips.push(ev(91, 2, 100 * SEC + SEC / 3, SEC, 1));
    let v = VideoRenderer::new(&p);
    let mut t = -SEC;
    while t < 112 * SEC {
        let (ix, scan) = v.active_ids_for_tests(Tick(t));
        assert_eq!(ix, scan, "at {t}");
        t += SEC / 50;
    }
}

/// A reel of `n` one-second events on one lane, placed `placements` times over its first second.
fn reel_scene(n: u64, placements: u64) -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, placements as u32);
    let mut s = Sequence::new(MediaId(100), "Reel");
    s.ensure_tracks(LaneKind::Video, 1);
    for i in 0..n {
        s.clips.push(ev(10_000 + i, 0, i as i64 * SEC, SEC, 1));
    }
    p.sequences.push(s);
    for k in 0..placements {
        p.clips.push(ev(1 + k, k as u32, 0, SEC, 100));
    }
    p.validate().expect("the reel scene is valid");
    p
}

/// t2663: the reel coverage bars cost the windows the placements show, once per window: not the
/// reel's events outside it, not once per placement (found-defects 154).
#[test]
fn t2663_reel_coverage_costs_the_window() {
    kt_plugins_builtin::install();
    let cache = cache();
    let walk = |n: u64, placements: u64| {
        let v = VideoRenderer::new(&reel_scene(n, placements));
        let (cov, c) = kt_time::cost::meter(|| v.nest_coverage(&cache, Q));
        assert_eq!(cov.len() as u64, placements);
        row(&c, Cost::FrameClip)
    };
    let base = walk(4, 1);
    assert!(base > 0);
    assert_eq!(walk(64, 1), base, "the reel's events outside the window were walked");
    assert_eq!(walk(4, 16), base, "the same window was walked once per placement");
}

/// t2664: a reel looked up by id inside a build answers by the index: the walk of a build with R
/// placed reels is linear in R, not R² (found-defects 157).
#[test]
fn t2664_a_reel_is_found_by_index() {
    kt_plugins_builtin::install();
    let scene = |r: u64| {
        let mut p = project();
        p.ensure_tracks(LaneKind::Video, 1);
        for i in 0..r {
            let mut s = Sequence::new(MediaId(100 + i), format!("R{i}"));
            s.ensure_tracks(LaneKind::Video, 1);
            s.clips.push(ev(10_000 + i, 0, 0, SEC, 1));
            p.sequences.push(s);
            p.clips.push(ev(1 + i, 0, i as i64 * SEC, SEC, 100 + i));
        }
        p
    };
    let passes = |r: u64| {
        let p = scene(r);
        row(&kt_model::without_profile_memo(|| count(|| drop(VideoRenderer::new(&p)))), Cost::ReelPass)
    };
    let (one, sixteen) = (passes(4), passes(64));
    assert!(sixteen <= 32 * one, "reel lookups grew {sixteen} against {one} for 16× the reels");
}

/// t2665: a staircase transition's third-event check looks at its lane near the window, in
/// validation and in a batch's reconcile (found-defects 156).
#[test]
fn t2665_a_staircase_check_costs_its_window() {
    let scene = |far: u64| {
        let mut p = project();
        p.ensure_tracks(LaneKind::Video, 1);
        p.clips.push(ev(1, 0, 0, 2 * SEC, 1));
        p.clips.push(ev(2, 0, SEC, 2 * SEC, 1));
        for i in 0..far {
            p.clips.push(ev(100 + i, 0, (10 + 2 * i as i64) * SEC, SEC, 1));
        }
        {
            p.transitions.push(TransitionEvent {
                id: TransitionId(1),
                a: Some(ClipId(1)),
                b: Some(ClipId(2)),
                in_offset: Tick(0),
                out_offset: Tick(0),
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
        p
    };
    let checks = |far: u64| {
        let p = scene(far);
        let reel = Indexed::new(&p);
        let c = count(|| {
            for _ in 0..8 {
                assert_eq!(check_transition_full(&reel, &p.transitions[0], None), Ok(()));
            }
        });
        row(&c, Cost::ClipSearch)
    };
    let base = checks(16);
    assert!(checks(256) <= base + 512, "the third-event check scanned the lane: {} against {base}", checks(256));
    // the index answers what the scan answers, the first event in the list's order
    let mut p = scene(4);
    p.clips.push(ev(50, 0, SEC + SEC / 2, SEC / 4, 1));
    p.clips.push(ev(51, 0, SEC + SEC / 4, SEC / 4, 1));
    let w = TickRange::new(Tick(SEC), Tick(2 * SEC));
    let scan = (&p as &dyn Reel).lane_busy(ClipKind::Video, 0, w, ClipId(1), ClipId(2));
    assert_eq!(Indexed::new(&p).lane_busy(ClipKind::Video, 0, w, ClipId(1), ClipId(2)), scan);
    assert_eq!(scan, Some(ClipId(50)));
}

/// t2666: a layer with live curves resolves its chain once per moment, shared by every plan of
/// that moment (the probe, the composite, a retry), and its values are the resolved ones
/// (found-defects 168).
#[test]
fn t2666_one_resolved_chain_per_moment() {
    kt_plugins_builtin::install();
    let cache = cache();
    let v = VideoRenderer::new(&scene(1));
    let t = Tick(SEC / 5);
    let first = count(|| {
        drop(v.probe_plan(&cache, t, Q));
        drop(v.frame_plan(&cache, t, Q));
        drop(v.frame_plan(&cache, t, Q));
    });
    assert_eq!(row(&first, Cost::ChainClone), 1, "three plans of one moment copied the chain more than once");
    let next = count(|| drop(v.frame_plan(&cache, Tick(t.0 + SEC / 25), Q)));
    assert_eq!(row(&next, Cost::ChainClone), 1, "a new moment resolves its chain");
    // the shared chain carries the curve's value at that moment
    let plan = v.frame_plan(&cache, Tick(SEC / 2), Q);
    let strength = plan.layers.iter().find_map(|l| match l {
        kt_engine::PlanLayer::One { src, .. } => src.fx.iter().find_map(|f| f.get_param("strength")),
        kt_engine::PlanLayer::Dissolve { a, .. } => a.fx.iter().find_map(|f| f.get_param("strength")),
    });
    let want = kt_model::curve_value(&curve("strength", 0.1, 0.9).keys, Tick(SEC / 2));
    assert_eq!(strength.map(|s| s as f32), Some(want as f32));
}

/// t2667: transitions, cameras and masks read their curves by compiled row: a frame sets no
/// parameter by name, and the values are the by-name ones bit for bit (found-defects 169).
#[test]
fn t2667_carriers_apply_curves_by_row() {
    kt_plugins_builtin::install();
    let cache = cache();
    let p = scene(1);
    let v = VideoRenderer::new(&p);
    let c = count(|| {
        for f in 0..50 {
            drop(v.frame_plan(&cache, Tick(f * SEC / 25), Q));
        }
    });
    assert_eq!(row(&c, Cost::ParamByName), 0, "a frame set a parameter by its name");
    // the compiled forms are the by-name forms
    let tr = &p.transitions[0];
    let cam = &p.cameras[0];
    let rows_t = RowCurves::compile(&tr.kind, &tr.auto);
    let rows_c = RowCurves::compile(&cam.cam, &cam.auto);
    let masks = MasksPlan::of(&p.clips[1].masks.picture);
    for i in 0..40 {
        let at = Tick(i * SEC / 10);
        let mut k = tr.kind.clone();
        rows_t.apply(&mut k, at);
        assert_eq!(k, tr.kind_at(at));
        assert_eq!(cam.params_at_rows(&rows_c, at), cam.params_at(at));
        assert_eq!(*masks.at(at), masks_at(&p.clips[1].masks.picture, at));
    }
}

/// t2668: the export's serial/pipelined choice builds no renderer and agrees with the build's
/// answer on events, reels and morpher layers (found-defects 191).
#[test]
fn t2668_the_stream_question_builds_nothing() {
    kt_plugins_builtin::install();
    let mosh = || FxInstance::plugin("datamosh").expect("datamosh is built in");
    let plain = scene(1);
    let mut on_event = scene(1);
    on_event.clips[0].fx.push(mosh());
    let mut in_reel = reel_scene(2, 1);
    in_reel.sequences[0].clips[0].fx.push(mosh());
    for p in [&plain, &on_event, &in_reel] {
        let (may, c) = kt_time::cost::meter(|| kt_engine::video::project_may_stream(p));
        assert_eq!(row(&c, Cost::RendererBuild), 0);
        assert_eq!(may, VideoRenderer::new(p).has_stream_stages());
    }
}
