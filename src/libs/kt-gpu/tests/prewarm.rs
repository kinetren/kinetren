//! t2120 (pipeline-prewarm; capabilities `video-plan`, `plugins`): after the prewarm of what a project
//! reaches, a frame compiles nothing — every video effect of the registry at its defaults and at each
//! preset, and the heavy stack, through the readback path and the preview's present path at 1080p,
//! draw no pipeline that was not warmed (`cold_draws`) and create none (`pipelines_in_frame`). Its own
//! test binary: the counters are process-wide, and its tests take `COUNTERS` in turn. Without a wgpu
//! adapter: skip with a printout.
//!
//! t2121: a geometry shape a frame meets for the first time compiles beside it: the frame runs the
//! interpreter (warm with the core), creates and colds nothing, and gives the bytes the shape's own
//! pipeline gives once the worker has it.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, PresentOptions, VideoGpuExec};
use kt_gpu::{cold_draws, pipelines_in_frame, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

static COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const W: u32 = 1920;
const H: u32 = 1080;

fn layer<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance], id: u64) -> LayerSrc<'a> {
    LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    }
}

fn plan<'a>(frames: &[Arc<VideoFrame>], fx: &'a [FxInstance], t: i64) -> FramePlan<'a> {
    let layers = frames
        .iter()
        .enumerate()
        .map(|(i, f)| PlanLayer::One {
            src: layer(f, if i + 1 == frames.len() { fx } else { &[] }, 10 + i as u64),
            alpha_k: if i == 0 { 4096 } else { 3000 },
            scale_alpha: false,
            fade: None,
        })
        .collect();
    FramePlan { t: Tick(t), out_w: W, out_h: H, layers }
}

fn instance(id: &str, sets: &[(&str, f64)]) -> FxInstance {
    let reg = kt_plugin_api::installed().unwrap();
    let m = reg.get(kt_plugin_api::Family::Fx, id).unwrap().manifest();
    let mut v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
    for (n, x) in sets {
        v[m.passport.iter().position(|s| s.name == *n).unwrap()] = *x;
    }
    FxInstance::plugin_with(id, &v).unwrap()
}

#[test]
fn t2120_after_prewarm_no_frame_compiles() {
    let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let frame = |alpha: bool, seed: u32| {
        let rgba: Vec<u8> = (0..W * H)
            .flat_map(|i| [(i % W) as u8 ^ seed as u8, (i / W) as u8, (i * 7 % 251) as u8, if alpha { (i % 200) as u8 + 55 } else { 255 }])
            .collect();
        Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
    };
    let frames = [frame(false, 1), frame(false, 2), frame(true, 3)];
    let reg = kt_plugin_api::installed().unwrap();
    let mut cases: Vec<(String, Vec<FxInstance>)> = vec![("no effect".into(), Vec::new())];
    for p in reg.plugins().filter(|p| p.manifest().media == kt_plugin_api::Media::VideoFx) {
        let m = p.manifest();
        if m.decl.video_class == Some(kt_plugin_api::VideoClass::Stream) {
            continue; // a stream stage runs on the compute host: t2122
        }
        cases.push((m.id.to_string(), vec![instance(&m.id, &[])]));
        for v in m.ui.variants {
            cases.push((format!("{} {}", m.id, v.name_key), vec![instance(&m.id, v.sets)]));
        }
    }
    cases.push((
        "the heavy stack".into(),
        vec![instance("vhs_tape", &[]), instance("camera_isp", &[]), instance("glow", &[]), instance("radial_blur", &[])],
    ));
    // what the player hands over for a project reaching every effect
    let ids: Vec<String> = cases.iter().flat_map(|(_, fx)| fx.iter().map(|f| f.tag().to_string())).collect();
    engine.prewarm(&ids);
    engine.settle_prewarm();
    let (cold, made) = (cold_draws(), pipelines_in_frame());
    let mut out = Vec::new();
    let mut t = 0;
    for (name, fx) in &cases {
        t += 3600;
        engine.execute_once(&plan(&frames, fx, t), &mut out).expect("execute");
        t += 3600;
        engine.present(&plan(&frames, fx, t), PresentOptions { show_incomplete: true, glow: true }).expect("present");
        assert_eq!(cold_draws() - cold, 0, "{name}: a pipeline drawn cold after the prewarm");
        assert_eq!(pipelines_in_frame() - made, 0, "{name}: a pipeline created inside a frame");
    }
    // a second prewarm of the same project asks nothing: the worker does not start
    engine.prewarm(&ids);
    engine.settle_prewarm();
    eprintln!("t2120: {} cases, nothing compiled in a frame", cases.len());
}

#[test]
fn t2121_a_new_geometry_shape_compiles_beside_the_frame() {
    let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    // the core, as a project's first prewarm asks it
    engine.prewarm(&[]);
    engine.settle_prewarm();
    let f = {
        let rgba: Vec<u8> = (0..W * H).flat_map(|i| [(i % W) as u8, (i / W) as u8, (i * 3 % 251) as u8, 255]).collect();
        Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false })
    };
    let frames = [f];
    let pan3d = FxInstance::plugin_with("pan3d", &[0.07, 0.0, 0.0, 11.0, 23.0, 4.0, 0.5, 0.5, 0.8, 0.0]).unwrap();
    let cases: Vec<Vec<FxInstance>> = vec![
        vec![FxInstance::geometry_at(0.031, -0.017, 1.37)],
        vec![pan3d.clone()],
        vec![FxInstance::geometry_at(0.02, 0.0, 0.61), pan3d],
    ];
    let mut t = 7_000_000;
    for (i, fx) in cases.iter().enumerate() {
        let shapes = engine.geometry_shapes();
        let (cold, made) = (cold_draws(), pipelines_in_frame());
        let (mut first, mut later) = (Vec::new(), Vec::new());
        t += 3600;
        engine.execute_once(&plan(&frames, fx, t), &mut first).expect("the first frame");
        assert_eq!(cold_draws() - cold, 0, "case {i}: the first frame drew a cold pipeline");
        assert_eq!(pipelines_in_frame() - made, 0, "case {i}: the first frame created a pipeline");
        engine.settle_prewarm();
        t += 3600;
        engine.execute_once(&plan(&frames, fx, t), &mut later).expect("the shape");
        assert!(first == later, "case {i}: the shape's bytes differ from the interpreter's");
        assert!(engine.geometry_shapes() >= shapes, "case {i}");
    }
    assert!(engine.geometry_shapes() >= 1, "no shape got a pipeline of its own");
}
