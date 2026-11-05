//! The VHS Tape's cost through the real engine (F-EXT-004, `--ignored`, a measurement): one 1080p
//! frame on the GPU graph.
//! `cargo test --release -p kt-gpu --test compute_vhs_cost -- --ignored --nocapture`.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::test_engine;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

#[test]
#[ignore]
fn bench_vhs_tape() {
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
    best("GPU graph with readback", &mut |t, o| assert!(engine.execute_into(&plan(t), o).unwrap()));
}

/// Frames per second of `KT_ENGINES` engines on as many threads, each with a device of its own:
/// a software adapter rasterizes one frame per queue. Run in a fresh process per count, with
/// `LP_NUM_THREADS` set to the share of cores of one engine.
#[test]
#[ignore]
fn bench_engines_in_parallel() {
    kt_plugins_builtin::install();
    let n: usize = std::env::var("KT_ENGINES").map_or(1, |v| v.parse().unwrap());
    let kind = std::env::var("KT_ENGINES_FX").unwrap_or("vhs_tape".into());
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
    let p = reg.get(kt_plugin_api::Family::Fx, &kind).expect("kind");
    let v: Vec<f64> = p.manifest().passport.iter().map(|s| s.default).collect();
    let fx = [FxInstance::plugin_with(&kind, &v).expect("kind")];
    let engines: Vec<_> = (0..n).map(|_| test_engine().expect("engine")).collect();
    let frames = 48 / n.max(1);
    let t0 = std::time::Instant::now();
    std::thread::scope(|s| {
        for (e, engine) in engines.iter().enumerate() {
            let (frame, fx) = (frame.clone(), &fx);
            s.spawn(move || {
                let mut out = Vec::new();
                for i in 0..frames {
                    let t = (3600 * (1 + i + e * frames)) as i64;
                    let plan = FramePlan {
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
                    assert!(engine.execute_into(&plan, &mut out).unwrap());
                }
            });
        }
    });
    let done = frames * n;
    println!("{kind} {n} engines: {:.1} frames/s", done as f64 / t0.elapsed().as_secs_f64());
}
