//! t1217 (camera-isp-simulation; capability `video-fx`): camera_isp's six-pass GPU graph against its
//! CPU kernel — the defaults and every preset, on opaque and alpha frames, within
//! `FrameTol::THRESHOLD` (the noise and the demosaic are byte mirrors; f32 `pow`/`exp`/division of
//! the device may move a rounding edge on a rare pixel), alpha frames included.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 96;

/// The tests of this file one at a time: the pass timer is process-wide, so a parallel test's camera
/// passes would land in t2311's sums (red at 0.81 beside t2310, 0.40 alone).
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}
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
    let (cpu, gpu) = run_frame(Some(engine), id, fx, frame(alpha));
    (cpu, gpu.expect("the GPU ran"))
}

/// One frame through the CPU composition and, given an engine, the GPU one.
fn run_frame(
    engine: Option<&dyn VideoGpuExec>,
    id: u64,
    fx: Vec<FxInstance>,
    fr: Arc<VideoFrame>,
) -> (Vec<u8>, Option<Vec<u8>>) {
    let (w, h) = (fr.width, fr.height);
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
        out_w: w,
        out_h: h,
        layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
    };
    let mut cpu = Vec::new();
    let cc = kt_gpu::reference::next(&mut cpu);
    let gpu = engine.map(|engine| {
        let mut gpu = Vec::new();
        let cg = engine.execute_into(&p, &mut gpu).expect("GPU execute");
        assert_eq!(cc, cg, "complete diverged");
        gpu
    });
    (cpu, gpu)
}

#[test]
fn t1217_the_gpu_graph_agrees_with_the_cpu_kernel() {
    let _one = lock();
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

/// t1430: the frame's edges are picture, not smears. The scene is a sawtooth of brightness along
/// each axis, rising 2 LSB per pixel from every edge inward with a period of 40 px, so every band
/// along an edge changes along the edge normal however the camera frames it. A stage that samples
/// beyond the recorded frame (the lens) or beyond the sensor (the ISP's lens correction) repeats
/// the edge texel along the normal — a flat profile. For every preset, the defaults and strong
/// distortion and lateral CA with mis-tuned corrections (|k| ≤ 0.3: a stronger pincushion folds,
/// and a fold is flat by optics), on the CPU and on the GPU: averaged along the middle third of
/// each edge, the output spans at least 1.5 LSB within every 4 px of depth from 1 to 28 px (4 px:
/// the mosaic's period — a Quad Bayer resolves each colour in 2×2 blocks and its demosaic leaves
/// plateaus of two pixels and a Gr/Gb step every four, inside the frame as well; a smear changes by
/// nothing — the ones this test guards were 7 to 44 px wide). The row noise and the
/// column fixed pattern are off on top of each form: an offset per row or column is the one defect
/// an average along the edge keeps, and it does not depend on where a stage samples.
#[test]
fn t1430_the_frame_edges_are_picture_not_smears() {
    let _one = lock();
    kt_plugins_builtin::install();
    let engine = test_engine();
    let reg = kt_plugin_api::installed().expect("installed");
    let cam = reg.get(kt_plugin_api::Family::Fx, "camera_isp").expect("camera_isp is built in");
    let m = cam.manifest();
    let defaults = kt_plugin_api::ParamValues::defaults(&m.passport).0;
    let row = |n: &str| m.passport.iter().position(|s| s.name == n).expect("a row");
    let (w, h) = (320u32, 180u32);
    let saw = |d: u32| 2 * (d % 40);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = (20 + saw(x.min(w - 1 - x)) + saw(y.min(h - 1 - y))) as u8;
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[v, v.saturating_sub(10), v.saturating_sub(20), 255]);
        }
    }
    let fr = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let mut forms = vec![("defaults".to_string(), defaults.clone())];
    for v in m.ui.variants {
        let mut vals = defaults.clone();
        for (n, x) in v.sets {
            vals[row(n)] = *x;
        }
        forms.push((v.name_key.to_string(), vals));
    }
    for (k, ca) in [(-0.3, 0.0), (0.3, 0.0), (0.0, 8.0), (0.0, -8.0), (-0.3, 6.0), (0.3, -8.0)] {
        for strength in [0.0, 0.5, 1.0, 2.0] {
            let mut vals = defaults.clone();
            vals[row("distortion")] = k;
            vals[row("lateral_ca")] = ca;
            vals[row("dewarp_strength")] = strength;
            vals[row("ca_corr_strength")] = strength;
            forms.push((format!("distortion {k} lateral CA {ca} corrections ×{strength}"), vals));
        }
    }
    // the luma of a pixel, mean of R, G, B
    let luma = |px: &[u8], x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        (px[i] as f64 + px[i + 1] as f64 + px[i + 2] as f64) / 3.0
    };
    // the mean profile of one edge by depth: (along, depth) → (x, y)
    let check = |px: &[u8], what: &str| {
        type At = Box<dyn Fn(u32, u32) -> (u32, u32)>;
        let edges: [(&str, u32, At); 4] = [
            ("left", h, Box::new(|a, d| (d, a))),
            ("right", h, Box::new(move |a, d| (w - 1 - d, a))),
            ("top", w, Box::new(|a, d| (a, d))),
            ("bottom", w, Box::new(move |a, d| (a, h - 1 - d))),
        ];
        for (edge, len, at) in &edges {
            let prof: Vec<f64> = (0..=28)
                .map(|d| {
                    let along = len / 3..len - len / 3;
                    let n = along.len() as f64;
                    along.map(|a| { let (x, y) = at(a, d); luma(px, x, y) }).sum::<f64>() / n
                })
                .collect();
            for d in 1..=24 {
                let win = &prof[d..=d + 4];
                let step = win.iter().cloned().fold(f64::MIN, f64::max)
                    - win.iter().cloned().fold(f64::MAX, f64::min);
                assert!(
                    step >= 1.5,
                    "{what}: the {edge} edge is flat at depth {d}–{} px (a change of {step:.2} LSB; profile {:?})",
                    d + 4,
                    prof.iter().map(|v| v.round() as i32).collect::<Vec<_>>()
                );
            }
        }
    };
    for (_, vals) in forms.iter_mut() {
        vals[row("row_noise")] = 0.0;
        vals[row("column_fpn")] = 0.0;
    }
    let mut id = 140_000;
    for (what, vals) in &forms {
        let f = FxInstance::plugin_with("camera_isp", vals).expect("the kind");
        id += 1;
        let (cpu, gpu) = run_frame(engine.as_deref().map(|e| e as &dyn VideoGpuExec), id, vec![f], fr.clone());
        check(&cpu, &format!("{what} (CPU)"));
        if let Some(gpu) = gpu {
            check(&gpu, &format!("{what} (GPU)"));
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
    let _one = lock();
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

/// One frame through the GPU with no memo (both forms share a plan key), at the frame's size.
fn gpu_bytes(engine: &kt_gpu::GpuFxEngine, id: u64, fx: &[FxInstance], fr: &Arc<VideoFrame>) -> Vec<u8> {
    let (w, h) = (fr.width, fr.height);
    let src = LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(fr.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    let p = FramePlan { t: Tick(90_000), out_w: w, out_h: h, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] };
    let mut out = Vec::new();
    engine.execute_measured(&p, &mut out).expect("GPU execute");
    out
}

/// A chain that runs `cam` behind a K× barrier: the grid squeezes the frame K×, the zoom brings it
/// back (the planner's K× rule), and the K the planner gave the camera.
fn at_k(cam: &FxInstance, k: u32, w: u32, h: u32) -> (Vec<FxInstance>, u32) {
    let fx = vec![
        FxInstance::plugin_with("event_grid", &[k as f64, k as f64, 0.0]).expect("the grid"),
        cam.clone(),
        FxInstance::geometry_zoom(k as f64),
    ];
    let plan = kt_fx::plan::plan_chain(&fx, w, h, 1, w, h, 0.5, None, 90_000, &[]);
    let got = plan
        .ops
        .iter()
        .find_map(|op| match op {
            kt_fx::plan::PlanOp::Barrier { idx: 1, res_scale } => Some(*res_scale),
            _ => None,
        })
        .unwrap_or(1);
    (fx, got)
}

/// t2310 (local-cost-queue, row 136): behind a K× barrier the camera's sensor passes render at the
/// sensor's size, and the frame is the frame-grid form's bytes (`cpu::with_general` keeps that form)
/// on the same device — the defaults, every preset, every layout × demosaic, opaque and alpha, K 2–4.
#[test]
fn t2310_the_sensor_grid_form_is_the_frame_grid_forms_bytes() {
    let _one = lock();
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
    for layout in 0..8 {
        for method in 0..3 {
            let mut vals = defaults.clone();
            vals[row("cfa")] = layout as f64;
            vals[row("demosaic")] = method as f64;
            forms.push((format!("cfa {layout} demosaic {method}"), vals));
        }
    }
    // the sensor graph renders its sensor passes at ⌈frame/K⌉, and only the last at the frame
    let mut nodes = Vec::new();
    let g = kt_plugin_api::barrier::GpuCtx { w: 194, h: 110, res_scale: 4, alpha: None, src_has_alpha: false, burst_index: 0 };
    cam.barrier().expect("a barrier").gpu_values(&defaults, &g, &mut nodes);
    let at: Vec<(u32, u32)> =
        nodes.iter().filter_map(|n| if let kt_plugin_api::barrier::Node::PassAt { w, h, .. } = n { Some((*w, *h)) } else { None }).collect();
    assert_eq!(at.len(), nodes.len() - 2, "every pass but the unprocessing and the last on the sensor grid: {nodes:?}");
    assert!(at.iter().all(|&s| s == (49, 28)), "the sensor's size is ⌈frame/K⌉: {at:?}");
    let mut id = 140_000;
    let mut differ = Vec::new();
    for k in [2u32, 3, 4] {
        for (what, vals) in &forms {
            let f = FxInstance::plugin_with("camera_isp", vals).expect("the kind");
            let (fx, got) = at_k(&f, k, W, H);
            assert_eq!(got, k, "the chain puts the camera at K {k}");
            for alpha in [false, true] {
                id += 1;
                let fr = frame(alpha);
                let new = gpu_bytes(&engine, id, &fx, &fr);
                let old = kt_plugin_api::cpu::with_general(|| gpu_bytes(&engine, id, &fx, &fr));
                if new != old {
                    let n = new.iter().zip(&old).filter(|(a, b)| a != b).count();
                    differ.push(format!("K {k} {what} alpha={alpha}: {n} bytes differ"));
                }
            }
        }
    }
    assert!(differ.is_empty(), "the sensor-grid form moved bytes: {differ:#?}");
}

/// t2311 (local-cost-queue, row 136): at K = 2 (960×540 logical, a 1920×1080 frame) the sensor-grid
/// form's camera passes take at most 60 % of the frame-grid form's device time — the pass timer's
/// sums, the forms interleaved, minima of five blocks. Skipped where the device has no timestamps.
#[test]
fn t2311_the_sensor_grid_form_costs_less() {
    let _one = lock();
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let (w, h) = (960u32, 540u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        p.copy_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, ((x ^ y) & 255) as u8, 255]);
    }
    let fr = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let cam = FxInstance::plugin("camera_isp").expect("the kind");
    let (fx, k) = at_k(&cam, 2, w, h);
    assert_eq!(k, 2);
    let camera_ns = |s: &kt_gpu::pass_time::Second| s.passes.iter().filter(|(n, ..)| n.starts_with("camera_isp:")).map(|(_, t, _)| *t).sum::<u64>();
    // warm both forms (a software device compiles at the first draw)
    gpu_bytes(&engine, 1, &fx, &fr);
    kt_plugin_api::cpu::with_general(|| gpu_bytes(&engine, 1, &fx, &fr));
    let _ = kt_gpu::pass_time::take();
    let (mut best_new, mut best_old) = (u64::MAX, u64::MAX);
    for _ in 0..5 {
        for general in [false, true] {
            let run = || {
                gpu_bytes(&engine, 1, &fx, &fr);
            };
            if general {
                kt_plugin_api::cpu::with_general(run);
            } else {
                run();
            }
            // a plain frame harvests the block's timestamps
            gpu_bytes(&engine, 2, &[], &fr);
            gpu_bytes(&engine, 2, &[], &fr);
            let Some(s) = kt_gpu::pass_time::take() else {
                println!("t2311: no pass timestamps on this device — skipped");
                return;
            };
            let ns = camera_ns(&s);
            if ns == 0 {
                println!("t2311: the camera's passes were not timed on this device — skipped");
                return;
            }
            if general {
                best_old = best_old.min(ns);
            } else {
                best_new = best_new.min(ns);
            }
        }
    }
    let ratio = best_new as f64 / best_old as f64;
    println!("t2311: camera passes {:.2} ms → {:.2} ms ({ratio:.2})", best_old as f64 / 1e6, best_new as f64 / 1e6);
    assert!(ratio <= 0.6, "the sensor-grid form costs {ratio:.2} of the frame-grid form's device time");
}
