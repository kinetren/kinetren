//! The VHS Tape's cost through the real engine (F-EXT-004, `--ignored`, a measurement): one 1080p
//! frame on the CPU kernel and on the GPU graph, on the same frame plan.
//! `cargo test --release -p kt-gpu --test compute_vhs_cost -- --ignored --nocapture`.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::test_engine;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

#[test]
#[ignore]
fn bench_vhs_tape_cpu_and_gpu() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let (w, h) = (1920u32, 1080u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        px[0] = (x / 4) as u8;
        px[1] = ((x ^ y) / 3) as u8;
        px[2] = (y / 3) as u8;
    }
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let reg = kt_plugin_api::installed().expect("installed");
    let p = reg.get(kt_plugin_api::Family::Fx, "vhs_tape").expect("vhs_tape");
    let v: Vec<f64> = p.manifest().passport.iter().map(|s| s.default).collect();
    let fx = [FxInstance::plugin_with("vhs_tape", &v).expect("vhs_tape")];
    let plan = |t: i64| FramePlan {
        t: Tick(t),
        out_w: w,
        out_h: h,
        layers: vec![PlanLayer::One {
            src: LayerSrc {
                source: SourceId(1),
                cache_idx: 0,
                content: t as u64,
                src_scale: 1,
                input: kt_engine::video_plan::LayerInput::Frame(Some(frame.clone())),
                fx: std::borrow::Cow::Borrowed(&fx[..]),
                fx_hash: t as u64,
                u: 0.5,
                cam3d: None,
                blend: None,
                blend_mode: kt_model::BlendMode::Normal,
                lead: None,
                masks: Vec::new(),
            },
            alpha_k: 4096,
            scale_alpha: false,
            fade: None,
        }],
        memoize_postfx: false,
    };
    let cache = BurstCache::new(CacheConfig::default());
    let mut out = Vec::new();
    let mut best = |name: &str, f: &mut dyn FnMut(i64, &mut Vec<u8>)| {
        let mut b = f64::MAX;
        for i in 0..8 {
            let t = std::time::Instant::now();
            f(3600 * (i + 1), &mut out);
            b = b.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("VHS Tape 1080p, {name}: {b:.1} ms");
    };
    best("CPU kernel (row bands, no SIMD)", &mut |t, o| assert!(compose_plan_cpu(&plan(t), &cache, o)));
    best("GPU graph with readback", &mut |t, o| assert!(engine.execute_into(&plan(t), o).unwrap()));
}
