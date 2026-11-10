//! `kt-cli pass-cost` (local-cost-queue): what a chain's passes cost on the device in the forms a
//! local fix chose between, on one headless engine. The forms are the specialized one (the product's)
//! and the general one (`kt_plugin_api::cpu::with_general`: the graph a barrier built before its fix),
//! run in interleaved blocks (the machine is shared: minima of blocks). It prints each pass label's
//! device time from the pass timer (gpu-pass-cost), the wall time of a frame with its readback, and
//! whether both forms gave the same bytes. Judge the numbers on a RELEASE build.
//!
//! `--form binds` compares instead a uniform write and a bind group per pass (the executor before
//! local-cost-queue's row 138) with the batched form, and adds the composing thread's CPU time.
//!
//! `--fx tag[:v,v,…][+tag…]` the chain (values in passport order, defaults past them), `--k K` runs
//! it behind a K× barrier (a grid squeezes the frame K×, a zoom brings it back), `--size WxH`
//! (default 1920x1080, the frame the chain gets), `--blocks N` (7), `--frames N` a block (3).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

fn plan<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance]) -> FramePlan<'a> {
    let src = LayerSrc {
        source: SourceId(11),
        cache_idx: 0,
        content: 11,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: 11,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    FramePlan { t: Tick(3000), out_w: f.width, out_h: f.height, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}

/// `tag[:v,v,…]` → the kind with those passport values.
fn kind(spec: &str) -> Result<FxInstance, String> {
    let (tag, vals) = spec.split_once(':').unwrap_or((spec, ""));
    let reg = kt_plugin_api::installed().ok_or("plugins not installed")?;
    let m = reg.get(kt_plugin_api::Family::Fx, tag).ok_or(format!("no video kind {tag}"))?.manifest();
    let mut v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
    for (i, x) in vals.split(',').filter(|s| !s.is_empty()).enumerate() {
        *v.get_mut(i).ok_or(format!("{tag} has {} rows", m.passport.len()))? = x.parse().map_err(|_| format!("not a number: {x}"))?;
    }
    FxInstance::plugin_with(tag, &v).ok_or(format!("{tag}: values out of range"))
}

pub fn run(args: &[String]) -> i32 {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let (w, h) = opt("--size")
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1920u32, 1080u32));
    let blocks: usize = opt("--blocks").and_then(|s| s.parse().ok()).unwrap_or(7);
    let per: usize = opt("--frames").and_then(|s| s.parse().ok()).unwrap_or(3);
    let k: u32 = opt("--k").and_then(|s| s.parse().ok()).unwrap_or(1);
    let binds = opt("--form").is_some_and(|f| f == "binds");
    let Some(spec) = opt("--fx") else {
        eprintln!("usage: kt-cli pass-cost --fx tag[:v,v,…][+tag…] [--k K] [--size WxH] [--blocks N] [--frames N]");
        return 2;
    };
    let mut fx = match spec.split('+').map(kind).collect::<Result<Vec<_>, _>>() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    if k > 1 {
        fx.insert(0, FxInstance::plugin_with("event_grid", &[k as f64, k as f64, 0.0]).expect("the grid"));
        fx.push(FxInstance::geometry_zoom(k as f64));
    }
    let engine = match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("no video device: {e}");
            return 1;
        }
    };
    println!("{} at {w}x{h}, K {k}, {blocks} blocks of {per} frames a form, minima", engine.info().name);
    let ids: Vec<String> = fx.iter().map(|f| f.tag().to_string()).collect();
    engine.prewarm_effects(&ids);
    engine.settle();
    let rgba: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i % w * 255 / w) as u8, (i / w * 255 / h) as u8, (((i % w) ^ (i / w)) & 0xff) as u8, 255])
        .collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let p = plan(&frame, &fx);
    let plain = plan(&frame, &[]);
    let mut out = [Vec::new(), Vec::new()];
    // (label → minimum device ns a frame, wall ms a frame) per form; a warm frame of each first (a
    // software device compiles a pipeline at its first draw)
    let mut best: [(BTreeMap<String, u64>, f64); 2] = Default::default();
    // the general form: the barriers' graphs before their fixes, or the per-pass executor
    let general = |f: &mut dyn FnMut()| {
        if binds {
            engine.per_pass_binds(true);
            f();
            engine.per_pass_binds(false);
        } else {
            kt_plugin_api::cpu::with_general(f)
        }
    };
    for form in 0..2 {
        let mut go = || {
            let _ = engine.execute_measured(&p, &mut Vec::new());
        };
        if form == 1 { general(&mut go) } else { go() }
    }
    let mut cpu = [f64::INFINITY; 2];
    let _ = kt_gpu::pass_time::take();
    for _ in 0..blocks {
        for (form, b) in best.iter_mut().enumerate() {
            let o = &mut out[form];
            let t = Instant::now();
            let c0 = thread_cpu();
            let mut block = || {
                for _ in 0..per {
                    engine.execute_measured(&p, o).expect("GPU execute");
                }
            };
            if form == 1 {
                general(&mut block);
            } else {
                block();
            }
            cpu[form] = cpu[form].min((thread_cpu() - c0) * 1e3 / per as f64);
            let wall = t.elapsed().as_secs_f64() * 1e3 / per as f64;
            let _ = engine.execute_measured(&plain, &mut Vec::new());
            let _ = engine.execute_measured(&plain, &mut Vec::new());
            let mut per_label: BTreeMap<String, u64> = BTreeMap::new();
            if let Some(s) = kt_gpu::pass_time::take() {
                for (n, t, _) in &s.passes {
                    *per_label.entry(n.clone()).or_default() += *t;
                }
            }
            for (n, t) in per_label {
                let v = t / per as u64;
                let e = b.0.entry(n).or_insert(u64::MAX);
                *e = (*e).min(v);
            }
            b.1 = if b.1 == 0.0 { wall } else { b.1.min(wall) };
        }
    }
    let labels: std::collections::BTreeSet<&String> = best[0].0.keys().chain(best[1].0.keys()).collect();
    println!("{:<28} {:>12} {:>12}", "pass", "general ms", "specialized ms");
    let (mut sum_g, mut sum_s) = (0u64, 0u64);
    for l in labels {
        let g = best[1].0.get(l).copied().unwrap_or(0);
        let s = best[0].0.get(l).copied().unwrap_or(0);
        sum_g += g;
        sum_s += s;
        println!("{l:<28} {:>12.2} {:>12.2}", g as f64 / 1e6, s as f64 / 1e6);
    }
    println!("{:<28} {:>12.2} {:>12.2}", "all passes", sum_g as f64 / 1e6, sum_s as f64 / 1e6);
    println!("{:<28} {:>12.1} {:>12.1}", "frame wall (readback)", best[1].1, best[0].1);
    println!("{:<28} {:>12.3} {:>12.3}", "composing thread CPU", cpu[1], cpu[0]);
    println!("same bytes: {}", out[0] == out[1]);
    0
}

/// This thread's CPU time (s); 0 where the platform gives none here.
#[cfg(unix)]
fn thread_cpu() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid clock id and a pointer to a live timespec
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

#[cfg(not(unix))]
fn thread_cpu() -> f64 {
    0.0
}
