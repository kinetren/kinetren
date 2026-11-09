//! `kt-cli fuse-cost` (fused-pointwise-runs): what a chain of point-wise effects costs fused against
//! unfused, on one headless engine. For each chain length it runs blocks of frames in each form,
//! interleaved (the machine is shared: minima of blocks), and prints the bytes the chain's passes move
//! per frame (a pass reads and writes the whole frame: device-independent), the device time of the
//! chain's passes from the pass timer (gpu-pass-cost) and the wall time of a frame with its readback.
//! It also checks that both forms give the same bytes. Judge the numbers on a RELEASE build.
//! `--size WxH` (default 1920x1080), `--stages 3,4,6`, `--blocks N` (default 7), `--frames N` a block
//! (default 4).

use std::sync::Arc;
use std::time::Instant;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::runs::FuseRuns;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const FUSING: [&str; 4] = ["color_grade", "posterize", "duotone", "vignette"];

fn plan<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance], w: u32, h: u32) -> FramePlan<'a> {
    let src = LayerSrc {
        source: SourceId(9),
        cache_idx: 0,
        content: 9,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: 9,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    FramePlan { t: Tick(3000), out_w: w, out_h: h, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}

/// The stage `i` of a chain: the colour family in turn, every numeric row a third of the way up.
fn stage(i: usize) -> FxInstance {
    let reg = kt_plugin_api::installed().expect("plugins installed");
    let id = FUSING[i % FUSING.len()];
    let m = reg.get(kt_plugin_api::Family::Fx, id).expect("a built-in").manifest();
    let v: Vec<f64> = m.passport.iter().map(|s| s.default + 0.33 * (s.max - s.default)).collect();
    FxInstance::plugin_with(id, &v).expect("values in range")
}

pub fn run(args: &[String]) -> i32 {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let (w, h) = opt("--size")
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1920u32, 1080u32));
    let stages: Vec<usize> =
        opt("--stages").map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|| vec![3, 4, 5, 6]);
    let blocks: usize = opt("--blocks").and_then(|s| s.parse().ok()).unwrap_or(7);
    let per: usize = opt("--frames").and_then(|s| s.parse().ok()).unwrap_or(4);
    let engine = match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("no video device: {e}");
            return 1;
        }
    };
    println!("{} at {w}x{h}, {blocks} blocks of {per} frames a form, minima", engine.info().name);
    let ids: Vec<String> = FUSING.iter().map(|s| s.to_string()).collect();
    engine.prewarm_effects(&ids);
    let rgba: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i % w * 255 / w) as u8, (i / w * 255 / h) as u8, (((i % w) ^ (i / w)) & 0xff) as u8, 255])
        .collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let plain = plan(&frame, &[], w, h);
    let px = w as f64 * h as f64;
    let mut out = Vec::new();
    for &k in &stages {
        let fx: Vec<FxInstance> = (0..k).map(stage).collect();
        let shape: Vec<String> = fx.iter().map(|f| f.tag().to_string()).collect();
        engine.prewarm_point_runs(&[shape]);
        engine.settle();
        let p = plan(&frame, &fx, w, h);
        // (device ns of the chain's passes a frame, wall ms a frame, chain passes, bytes) per form
        let mut best = [(f64::INFINITY, f64::INFINITY, 0u32, Vec::new()), (f64::INFINITY, f64::INFINITY, 0u32, Vec::new())];
        for _ in 0..blocks {
            for (form, mode) in [(0usize, FuseRuns::Never), (1, FuseRuns::Always)] {
                engine.fuse_runs(mode);
                let _ = kt_gpu::pass_time::take();
                let t = Instant::now();
                let mut passes = 0;
                for _ in 0..per {
                    let (_, cost) = engine.execute_measured(&p, &mut out).expect("GPU execute");
                    passes = cost.passes;
                }
                let wall = t.elapsed().as_secs_f64() * 1e3 / per as f64;
                // one plain frame collects the block's timestamps: its readback waits for the device,
                // and the harvest after it reads the maps that came back
                let _ = engine.execute_measured(&plain, &mut Vec::new());
                let _ = engine.execute_measured(&plain, &mut Vec::new());
                let dev = kt_gpu::pass_time::take().map_or(f64::NAN, |s| {
                    let chain: u64 =
                        s.passes.iter().filter(|(n, ..)| FUSING.contains(&n.as_str()) || n.starts_with("pointwise:")).map(|(_, t, _)| *t).sum();
                    chain as f64 / per as f64
                });
                let b = &mut best[form];
                b.0 = b.0.min(dev);
                b.1 = b.1.min(wall);
                b.2 = passes;
                b.3 = out.clone();
            }
        }
        engine.fuse_runs(FuseRuns::Measured);
        // the chain's passes (`FrameCost.passes`, the composite apart) each read and write the frame:
        // 8 bytes a pixel
        let mb = |passes: u32| passes as f64 * 8.0 * px / 1e6;
        println!(
            "{k} stages: unfused {} passes {:.1} MB, chain {:.2} ms device, frame {:.1} ms | fused {} passes {:.1} MB, chain {:.2} ms device, frame {:.1} ms | same bytes: {}",
            best[0].2,
            mb(best[0].2),
            best[0].0 / 1e6,
            best[0].1,
            best[1].2,
            mb(best[1].2),
            best[1].0 / 1e6,
            best[1].1,
            best[0].3 == best[1].3
        );
    }
    let (ready, fused, split) = engine.run_shapes();
    println!("run shapes ready {ready}, the trial kept fused {fused}, split {split}");
    0
}
