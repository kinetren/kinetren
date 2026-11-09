//! fused-pointwise-runs (capability `video-plan`): a run of point-wise stages is one generated pass.
//!
//! t2190: every ordered pair of fusable point-wise plugins (defaults and a tuned set), a six-stage run,
//! a 40-stage run (over the uniform's room: two pieces), a run inside a strength slot and runs beside a
//! geometry series compose at 1080p fused and with fusion off; each fused frame is within 1 LSB per
//! former boundary of the unfused one, and byte-equal on a software adapter; the fused frame runs one
//! pass per piece.
//!
//! t2192: a run shape a frame meets for the first time compiles beside it: the frame runs the parts,
//! draws nothing cold and creates no pipeline; after the worker settles the shape is ready and the next
//! frame runs one pass with the first frame's bytes; a shape handed to `prewarm_point_runs` is ready
//! before its first frame.
//!
//! t2197: preview equals export with a run, on a fused engine and on one with fusion off.
//!
//! Its own test binary: the cold-draw counters are process-wide, every test takes `COUNTERS`.
//! Without a wgpu adapter: skip with a printout.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kt_cache::SourceId;
use kt_engine::{ExportBackends, FramePlan, LayerSrc, PlanLayer, VideoGpuExec, VideoRenderer};
use kt_gpu::runs::FuseRuns;
use kt_gpu::{FrameTol, GpuFxEngine, assert_frames_close, cold_draws, pipelines_in_frame, test_engine};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;

static COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const W: u32 = 1920;
const H: u32 = 1080;

const FUSING: [&str; 4] = ["color_grade", "duotone", "posterize", "vignette"];

fn frame(alpha: bool) -> Arc<VideoFrame> {
    let rgba: Vec<u8> = (0..W * H)
        .flat_map(|i| {
            let (x, y) = (i % W, i / W);
            [(x * 255 / W) as u8, (y * 255 / H) as u8, ((x ^ y) & 0xff) as u8, if alpha { (x % 200) as u8 + 55 } else { 255 }]
        })
        .collect();
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

fn plan<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance]) -> FramePlan<'a> {
    let src = LayerSrc {
        source: SourceId(5),
        cache_idx: 0,
        content: 5,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: 5,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    FramePlan { t: Tick(3000), out_w: W, out_h: H, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}

/// The plugin at its defaults, or with every numeric row moved 37 % of the way to its top.
fn instance(id: &str, tuned: bool) -> FxInstance {
    let reg = kt_plugin_api::installed().unwrap();
    let m = reg.get(kt_plugin_api::Family::Fx, id).unwrap().manifest();
    let v: Vec<f64> = m.passport.iter().map(|s| if tuned { s.default + 0.37 * (s.max - s.default) } else { s.default }).collect();
    FxInstance::plugin_with(id, &v).unwrap()
}

fn compose(e: &GpuFxEngine, f: &Arc<VideoFrame>, fx: &[FxInstance]) -> (Vec<u8>, u32) {
    let mut out = Vec::new();
    let (complete, cost) = e.execute_measured(&plan(f, fx), &mut out).expect("GPU execute");
    assert!(complete);
    (out, cost.passes)
}

/// A warm engine with every fusable plugin and the core.
fn warm_engine() -> Option<Arc<GpuFxEngine>> {
    kt_plugins_builtin::install();
    let e = test_engine()?;
    let ids: Vec<String> = FUSING.iter().map(|s| s.to_string()).chain(["blur".to_string()]).collect();
    e.prewarm_effects(&ids);
    e.settle();
    Some(e)
}

/// Fused against unfused: the bound is 1 LSB per former boundary (the owner's agreement, H16); on a
/// software adapter the two are byte-equal, since the fused text rounds after every stage as the own
/// pass stores it.
fn check(e: &GpuFxEngine, f: &Arc<VideoFrame>, fx: &[FxInstance], boundaries: u32, saved: u32, what: &str) {
    e.fuse_runs(FuseRuns::Never);
    let (unfused, up) = compose(e, f, fx);
    e.fuse_runs(FuseRuns::Always);
    let _ = compose(e, f, fx); // a shape seen first runs its parts while it compiles
    e.settle();
    let (fused, fp) = compose(e, f, fx);
    e.fuse_runs(FuseRuns::Measured);
    assert_eq!(up - fp, saved, "{what}: the fused frame runs one pass per piece ({up} unfused, {fp} fused)");
    let tol = FrameTol { max_lsb: boundaries, outlier_frac: 0.0, outlier_max: boundaries, mean_lsb: boundaries as f64 };
    assert_frames_close(&unfused, &fused, W, H, tol, what);
    if e.info().software {
        assert!(fused == unfused, "{what}: on a software adapter the fused frame is the unfused one byte for byte");
    }
}

#[test]
fn t2190_fused_equals_unfused_over_every_pair_and_a_long_run() {
    let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(e) = warm_engine() else { return };
    let (opaque, alpha) = (frame(false), frame(true));
    for tuned in [false, true] {
        for a in FUSING {
            for b in FUSING {
                let fx = vec![instance(a, tuned), instance(b, !tuned)];
                check(&e, if tuned { &alpha } else { &opaque }, &fx, 1, 1, &format!("{a}+{b} tuned {tuned}"));
            }
        }
    }
    // six stages, tuned and default mixed, an audio slot inside
    let six = vec![
        instance("color_grade", true),
        instance("posterize", false),
        FxInstance::plugin("gain").unwrap(),
        instance("vignette", true),
        instance("duotone", true),
        instance("color_grade", false),
        instance("posterize", true),
    ];
    check(&e, &opaque, &six, 5, 5, "six stages");
    // 40 stages: 32 in the first piece, 8 in the second
    let forty: Vec<FxInstance> = (0..40).map(|i| instance(FUSING[i % 4], i % 3 == 0)).collect();
    check(&e, &alpha, &forty, 38, 38, "forty stages");
    // inside a strength slot, and beside a geometry series
    let slot = vec![
        instance("posterize", true),
        instance("color_grade", true),
        instance("vignette", true),
        FxInstance::LayerMix { amount: 0.6, from: 1 },
    ];
    check(&e, &opaque, &slot, 1, 1, "a run inside a strength slot");
    let beside = vec![
        instance("duotone", true),
        instance("posterize", true),
        FxInstance::geometry_at(0.05, -0.02, 1.15),
        FxInstance::blur(3),
        instance("vignette", true),
        instance("color_grade", true),
    ];
    check(&e, &opaque, &beside, 2, 2, "runs beside a geometry series and a barrier");
}

#[test]
fn t2192_a_run_shape_compiles_beside_its_frame() {
    let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(e) = warm_engine() else { return };
    let f = frame(false);
    let fx = vec![instance("color_grade", true), instance("posterize", true), instance("vignette", true)];
    let (cold, ins) = (cold_draws(), pipelines_in_frame());
    let (first, p1) = compose(&e, &f, &fx);
    assert_eq!((cold_draws(), pipelines_in_frame()), (cold, ins), "the first frame of a run shape draws nothing cold, creates nothing");
    e.settle();
    assert!(e.run_shapes().0 >= 1, "the worker compiled the shape");
    e.fuse_runs(FuseRuns::Always);
    let (next, p2) = compose(&e, &f, &fx);
    e.fuse_runs(FuseRuns::Measured);
    assert_eq!(p1 - p2, 2, "the next frame runs the three stages as one pass");
    assert!(next == first || !e.info().software, "the fused frame gives the first frame's bytes");
    assert_eq!((cold_draws(), pipelines_in_frame()), (cold, ins), "a ready shape draws nothing cold");
    // prewarmed: ready before its first frame
    let before = e.run_shapes().0;
    e.prewarm_point_runs(&[vec!["duotone".into(), "vignette".into()]]);
    e.settle();
    assert_eq!(e.run_shapes().0, before + 1);
    e.fuse_runs(FuseRuns::Always);
    let (_, p) = compose(&e, &f, &[instance("duotone", true), instance("vignette", false)]);
    e.fuse_runs(FuseRuns::Measured);
    let (_, own) = {
        e.fuse_runs(FuseRuns::Never);
        let r = compose(&e, &f, &[instance("duotone", true), instance("vignette", false)]);
        e.fuse_runs(FuseRuns::Measured);
        r
    };
    assert_eq!(own - p, 1, "a prewarmed shape's first frame runs fused");
    assert_eq!((cold_draws(), pipelines_in_frame()), (cold, ins));
    // the product's mode: a warm shape is tried on interleaved frames until a verdict; every frame
    // of the trial gives the same bytes (the switch moves no stored frame)
    let (_, fused0, split0) = e.run_shapes();
    let mut n = 0;
    while e.run_shapes().1 + e.run_shapes().2 == fused0 + split0 {
        let (out, _) = compose(&e, &f, &fx);
        assert!(out == first || !e.info().software, "a trial frame differs from the reference bytes");
        n += 1;
        assert!(n <= 2 * 64 + 16, "the trial never ended");
    }
    let (_, fused1, split1) = e.run_shapes();
    eprintln!("t2192: the trial decided after {n} frames: fused {} split {}", fused1 - fused0, split1 - split0);
}

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips = vec![ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![instance("color_grade", true), instance("posterize", true), instance("duotone", true), instance("vignette", true)],
        opacity: 1.0,
        fx_off: false,
    }];
    p.normalize();
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)) as Box<dyn MediaSource>)]
}

#[test]
fn t2197_preview_equals_export_with_a_run() {
    let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    kt_plugins_builtin::install();
    let p = project();
    let q = Quality { width: 128, height: 72 };
    let target = 11usize;
    let mut frames = Vec::new();
    let mut software = false;
    for mode in [FuseRuns::Always, FuseRuns::Never] {
        let Some(e) = test_engine() else { return };
        software = e.info().software;
        e.fuse_runs(mode);
        // the raw export path: the first frames run the parts while the shape compiles
        let backends = ExportBackends { video_gpu: Some(e.clone()), ..Default::default() };
        let mut got = Vec::new();
        let mut n = 0usize;
        kt_engine::export::render_raw_video_range_with(&p, sources(), q, None, &backends, &Arc::new(AtomicBool::new(false)), |_| {}, |rgba| {
            if n == target {
                got = rgba.to_vec();
            }
            n += 1;
            Ok(())
        })
        .unwrap();
        frames.push(got);
        // the preview of the same moment on a fresh engine of the same mode
        let Some(e2) = test_engine() else { return };
        e2.fuse_runs(mode);
        let video = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let t = Tick(target as i64 * 3000);
        let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> = sources().into_iter().collect();
        for (media, pts, q) in video.layers_at(t, q) {
            let fr = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(q)).unwrap();
            cache.insert_video(kt_engine::source_id(media), Some(q), kt_engine::video::video_cache_idx(pts), fr.into());
        }
        e2.prewarm_runs(&kt_engine::prewarm::run_list(&kt_engine::prewarm::run_shapes(&p)));
        e2.settle();
        let mut out = Vec::new();
        assert!(e2.execute_into(&video.frame_plan(&cache, t, q), &mut out).unwrap());
        frames.push(out);
    }
    if !software {
        for (i, f) in frames.iter().enumerate() {
            assert_frames_close(&frames[0], f, q.width, q.height, FrameTol { max_lsb: 3, outlier_frac: 0.0, outlier_max: 3, mean_lsb: 3.0 }, &format!("frame {i}"));
        }
        return;
    }
    assert!(frames.iter().all(|f| *f == frames[0]), "export and preview, fused and unfused, give one frame");
}
