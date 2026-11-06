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
                    };
                    assert!(engine.execute_into(&plan, &mut out).unwrap());
                }
            });
        }
    });
    let done = frames * n;
    println!("{kind} {n} engines: {:.1} frames/s", done as f64 / t0.elapsed().as_secs_f64());
}

/// What a generator costs at 1080p on the CPU, against the floor of any shader: one compute job
/// with a frame's worth of readback (generators round, `--ignored`, a measurement).
#[test]
#[ignore]
fn bench_generators_against_a_compute_frame() {
    use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let (w, h) = (1920u32, 1080u32);
    let reg = kt_plugin_api::installed().expect("installed");
    for p in reg.plugins() {
        let Some(g) = p.generator() else { continue };
        if g.synth().is_some() {
            continue;
        }
        let vals: Vec<f64> = p.manifest().passport.iter().map(|s| s.default).collect();
        let mut best = f64::MAX;
        for i in 0..6 {
            let t = std::time::Instant::now();
            std::hint::black_box(g.frame(&vals, Tick(3600 * i), w, h));
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{:>16} {best:8.2} ms", p.manifest().id);
    }
    let host = kt_gpu::compute::open_device().expect("device");
    let shader = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
        @group(0) @binding(1) var<storage, read_write> out: array<u32>;\n\
        @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
        let n = u.x * u.y; for (var i = id.x * 256u; i < min(n, id.x * 256u + 256u); i++) { var x = i * 2654435761u + u.z; x ^= x >> 13u; x *= 2654435761u; out[i] = x | 0xff000000u; } }\n";
    let bytes = (w * h * 4) as u64;
    let mut best = f64::MAX;
    for i in 0..6u32 {
        let uniform: Vec<u8> = [w, h, i, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let step = Step { shader, uniform: &uniform, read: &[], write: &[Buf(0)], groups: [(w * h).div_ceil(256 * 64), 1, 1] };
        let t = std::time::Instant::now();
        let got = host.run(&Job { buffers: &[Init::Zero(bytes)], steps: &[step], reads: &[Buf(0)] }).unwrap();
        std::hint::black_box(got);
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    println!("{:>16} {best:8.2} ms", "compute frame");
}
