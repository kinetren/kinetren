//! `kt-cli first-frame` (pipeline-prewarm): what the first frame of each video effect costs against
//! its warm frames, on one headless engine at the project size. A first frame that costs more than
//! the warm ones is a pipeline compiled inside it (by us or lazily by the driver). Also: the engine's
//! build, a second build on its own device, `--prewarm` (every effect warmed first, as the player
//! does for what a project reaches) and `--compute` (the compute host's opening and each declared
//! shader's preparation: what a first Datamosh or generator frame paid inside it before).
//! Judge the numbers on a RELEASE build; an empty Mesa cache is `MESA_SHADER_CACHE_DIR=<new dir>`.

use std::sync::Arc;
use std::time::Instant;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

pub fn run(args: &[String]) -> i32 {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let (w, h) = opt("--size")
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1920u32, 1080u32));
    let only = opt("--kind").cloned();
    if args.iter().any(|a| a == "--kept") {
        // the editor's case: a project opened again builds its engine on the same display device
        let ctx = match kt_gpu::GpuContext::new_headless() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("no video device: {e}");
                return 1;
            }
        };
        let again = || kt_gpu::GpuContext::from_parts(ctx.device.clone(), ctx.queue.clone(), ctx.adapter_info.clone());
        for n in ["first", "second"] {
            let t = Instant::now();
            let ok = kt_gpu::GpuFxEngine::new_kept(again()).is_ok();
            println!("{n} engine on the kept display device {:.1} ms{}", ms(t), if ok { "" } else { " (failed)" });
        }
    }
    let t = Instant::now();
    let engine = match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("no video device: {e}");
            return 1;
        }
    };
    println!("engine build {:.1} ms on {}", ms(t), engine.info().name);
    if args.iter().any(|a| a == "--rebuild") {
        let t = Instant::now();
        let again = kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new);
        println!("a second engine (its own device) {:.1} ms{}", ms(t), if again.is_ok() { "" } else { " (failed)" });
    }
    let reg = kt_plugin_api::installed().expect("plugins installed");
    if args.iter().any(|a| a == "--prewarm") {
        // what the player asks when a project reaches every effect (pipeline-prewarm D5)
        let ids: Vec<String> = reg.plugins().map(|p| p.manifest().id.to_string()).collect();
        let t = Instant::now();
        engine.prewarm_effects(&ids);
        engine.settle();
        println!("prewarm of every effect {:.1} ms (on the warm worker; the UI never waits for it)", ms(t));
    }
    if args.iter().any(|a| a == "--compute") {
        // what a first Datamosh or generator frame paid inside it before: the device, then each shader
        let t = Instant::now();
        let host = kt_plugin_api::compute::host();
        println!("compute host open {:.1} ms{}", ms(t), if host.is_some() { "" } else { " (none)" });
        if let Some(h) = host {
            for p in reg.plugins() {
                for sh in p.compute_shaders() {
                    let t = Instant::now();
                    let r = h.prepare(&sh);
                    println!("  {:<20} shader of {:>5} bytes prepared {:>8.1} ms{}", p.manifest().id, sh.text.len(), ms(t), if r.is_ok() { "" } else { " (refused)" });
                }
            }
        }
    }
    let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i % w) as u8, (i / w) as u8, (i * 7 % 251) as u8, 255]).collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let mut out = Vec::new();
    let mut tick = 0i64;
    let mut compose = |fx: &[FxInstance]| {
        tick += 3600;
        let src = LayerSrc {
            source: SourceId(7),
            cache_idx: 0,
            content: 7,
            src_scale: 1,
            input: kt_engine::video_plan::LayerInput::Frame(Some(frame.clone())),
            fx: std::borrow::Cow::Borrowed(fx),
            fx_hash: tick as u64,
            u: 0.5,
            cam3d: None,
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead: None,
            masks: Vec::new(),
        };
        let plan = FramePlan {
            t: Tick(tick),
            out_w: w,
            out_h: h,
            layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
        };
        let t = Instant::now();
        let _ = engine.execute_once(&plan, &mut out);
        ms(t)
    };
    // the bare composite first: what every effect's frame also pays
    let bare = compose(&[]);
    let bare_warm = (0..3).map(|_| compose(&[])).fold(f64::INFINITY, f64::min);
    println!("{:<24} first {:>8.1} ms  warm {:>7.1} ms", "(no effect)", bare, bare_warm);
    let (mut sum_first, mut sum_warm, mut worst) = (0.0, 0.0, (0.0, String::new()));
    for p in reg.plugins() {
        let m = p.manifest();
        if m.media != kt_plugin_api::Media::VideoFx
            || m.decl.video_class == Some(kt_plugin_api::VideoClass::Stream)
            || only.as_deref().is_some_and(|k| k != m.id)
        {
            continue;
        }
        let v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
        let Some(fx) = FxInstance::plugin_with(&m.id, &v) else { continue };
        let fx = [fx];
        let first = compose(&fx);
        let warm = (0..3).map(|_| compose(&fx)).fold(f64::INFINITY, f64::min);
        println!("{:<24} first {:>8.1} ms  warm {:>7.1} ms  extra {:>8.1} ms", m.id, first, warm, first - warm);
        sum_first += first;
        sum_warm += warm;
        if first - warm > worst.0 {
            worst = (first - warm, m.id.to_string());
        }
    }
    println!(
        "all effects: first frames {:.0} ms, warm {:.0} ms, extra {:.0} ms; worst {} +{:.0} ms; geometry shapes {}; cold draws {}",
        sum_first,
        sum_warm,
        sum_first - sum_warm,
        worst.1,
        worst.0,
        engine.geometry_shapes(),
        kt_gpu::cold_draws()
    );
    0
}
