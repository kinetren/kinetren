//! t1217 (camera-isp-simulation; capability `video-fx`): camera_isp's six-pass GPU graph against its
//! CPU kernel — the defaults and every preset, on opaque and alpha frames, within
//! `FrameTol::THRESHOLD` (the noise and the demosaic are byte mirrors; f32 `pow`/`exp`/division of
//! the device may move a rounding edge on a rare pixel), alpha frames included.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 96;
const H: u32 = 54;

fn frame(alpha: bool) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let px = if x > W / 2 && y > H / 2 {
                [30, 200, 40]
            } else if (x / 8 + y / 8) % 2 == 0 {
                [(x * 2) as u8, (y * 4) as u8, ((x + y) * 2) as u8]
            } else {
                [230, 210 - (y * 2) as u8, 50]
            };
            rgba[i..i + 3].copy_from_slice(&px);
            rgba[i + 3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

fn run(engine: &dyn VideoGpuExec, id: u64, fx: Vec<FxInstance>, alpha: bool) -> (Vec<u8>, Vec<u8>) {
    let fr = frame(alpha);
    let fx_hash = xxhash_rust::xxh3::xxh3_64(format!("{fx:?}|{id}").as_bytes());
    let src = LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: xxhash_rust::xxh3::xxh3_64(&fr.rgba),
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(fr)),
        fx: std::borrow::Cow::Owned(fx),
        fx_hash,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    let p = FramePlan {
        t: Tick(90_000),
        out_w: W,
        out_h: H,
        layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
        memoize_postfx: true,
    };
    let cache = BurstCache::new(CacheConfig::default());
    let (mut cpu, mut gpu) = (Vec::new(), Vec::new());
    let cc = compose_plan_cpu(&p, &cache, &mut cpu);
    let cg = engine.execute_into(&p, &mut gpu).expect("GPU execute");
    assert_eq!(cc, cg, "complete diverged");
    (cpu, gpu)
}

#[test]
fn t1217_the_gpu_graph_agrees_with_the_cpu_kernel() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let reg = kt_plugin_api::installed().expect("installed");
    let cam = reg.get(kt_plugin_api::Family::Fx, "camera_isp").expect("camera_isp is built in");
    let m = cam.manifest();
    let defaults = kt_plugin_api::ParamValues::defaults(&m.passport).0;
    let row = |n: &str| m.passport.iter().position(|s| s.name == n).expect("a row");
    let mut forms = vec![("defaults".to_string(), defaults.clone())];
    for v in m.ui.variants {
        let mut vals = defaults.clone();
        for (n, x) in v.sets {
            vals[row(n)] = *x;
        }
        forms.push((v.name_key.to_string(), vals));
    }
    // every layout and demosaic once more, on the defaults
    for layout in 0..8 {
        for method in 0..3 {
            let mut vals = defaults.clone();
            vals[row("cfa")] = layout as f64;
            vals[row("demosaic")] = method as f64;
            forms.push((format!("cfa {layout} demosaic {method}"), vals));
        }
    }
    let mut id = 120_000;
    for (what, vals) in &forms {
        let f = FxInstance::plugin_with("camera_isp", vals).expect("the kind");
        for alpha in [false, true] {
            id += 1;
            let (cpu, gpu) = run(engine.as_ref(), id, vec![f.clone()], alpha);
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, &format!("camera_isp {what} alpha={alpha}"));
        }
    }
}

/// The cost of camera_isp's graph per 1080p frame on this machine's device (design D9). Not a
/// regression test: `cargo test -p kt-gpu --test camera_isp -- --ignored --nocapture`. Each frame
/// is a new plan (another content), so no memo answers it; the time includes the upload and the
/// readback.
#[test]
#[ignore]
fn camera_isp_gpu_cost_per_1080p_frame() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let (w, h) = (1920u32, 1080u32);
    let reg = kt_plugin_api::installed().expect("installed");
    let m = reg.get(kt_plugin_api::Family::Fx, "camera_isp").expect("camera_isp").manifest();
    let vals = kt_plugin_api::ParamValues::defaults(&m.passport).0;
    let f = FxInstance::plugin_with("camera_isp", &vals).expect("the kind");
    let run = |id: u64| {
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i % w) as u8, (i / w) as u8, (i / 7 + id as u32) as u8, 255]).collect();
        let fr = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
        let src = LayerSrc {
            source: SourceId(id),
            cache_idx: 0,
            content: id,
            src_scale: 1,
            input: kt_engine::video_plan::LayerInput::Frame(Some(fr)),
            fx: std::borrow::Cow::Owned(vec![f.clone()]),
            fx_hash: id,
            u: 0.5,
            cam3d: None,
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead: None,
            masks: Vec::new(),
        };
        let p = FramePlan {
            t: Tick(90_000 + id as i64),
            out_w: w,
            out_h: h,
            layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
            memoize_postfx: false,
        };
        let mut out = Vec::new();
        engine.execute_into(&p, &mut out).expect("GPU execute");
    };
    run(900_000);
    let t = std::time::Instant::now();
    for i in 1..=5 {
        run(900_000 + i);
    }
    println!("camera_isp on the GPU: {:.0} ms/frame (1080p, defaults, with upload and readback)", t.elapsed().as_secs_f64() * 200.0);
}
