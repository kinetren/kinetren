//! t1186 (vhs-tape-simulation; capability `video-fx`): the VHS tape simulation's GPU graph — the
//! parameter upload, the line table, eight stages on the standard's signal grid and the frame pass
//! — is within `FrameTol::SIGNAL` of the CPU reference, at its defaults and at each preset, PAL and
//! NTSC, alone and K×2 between geometry, on a frame with alpha; and it is deterministic on the
//! device. Without a wgpu adapter: skip with a printout (KT_REQUIRE_GPU=1 requires one).

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 192;
const H: u32 = 108;

/// Colour bars over a ramp with fine detail, optionally with a gradient alpha.
fn frame(alpha: bool) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    let bars = [[235, 235, 235], [235, 235, 16], [16, 235, 235], [16, 235, 16], [235, 16, 235], [235, 16, 16], [16, 16, 235], [16, 16, 16]];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let c = if y < H / 2 {
                bars[(x * 8 / W) as usize]
            } else {
                let v = (x * 255 / (W - 1)) as u8;
                let fine = if (x / 2 + y / 2) % 2 == 0 { 30 } else { 0 };
                [v, v.saturating_add(fine), 255 - v]
            };
            rgba[i..i + 3].copy_from_slice(&c);
            rgba[i + 3] = if alpha { (x * 255 / (W - 1)) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

struct Src {
    frame: Arc<VideoFrame>,
    fx: Vec<FxInstance>,
    id: u64,
}

impl Src {
    fn layer(&self) -> LayerSrc<'_> {
        let repr = format!("{:?}|{}", self.fx, self.id);
        LayerSrc {
            source: SourceId(self.id),
            cache_idx: 0,
            content: xxhash_rust::xxh3::xxh3_64(&self.frame.rgba),
            src_scale: 1,
            input: kt_engine::video_plan::LayerInput::Frame(Some(self.frame.clone())),
            fx: std::borrow::Cow::Borrowed(&self.fx[..]),
            fx_hash: xxhash_rust::xxh3::xxh3_64(repr.as_bytes()),
            u: 0.5,
            cam3d: None,
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead: None,
            masks: Vec::new(),
        }
    }
}

fn run(engine: &dyn VideoGpuExec, src: &Src, t: i64) -> (Vec<u8>, Vec<u8>) {
    let p = FramePlan {
        t: Tick(t),
        out_w: W,
        out_h: H,
        layers: vec![PlanLayer::One { src: src.layer(), alpha_k: 4096, scale_alpha: false, fade: None }],
        memoize_postfx: false,
    };
    let cache = BurstCache::new(CacheConfig::default());
    let mut cpu = Vec::new();
    assert!(compose_plan_cpu(&p, &cache, &mut cpu), "CPU incomplete");
    let mut gpu = Vec::new();
    assert!(engine.execute_into(&p, &mut gpu).expect("GPU execute"), "GPU incomplete");
    (cpu, gpu)
}

/// The vhs_tape slot with `sets` applied by row name over its defaults.
fn tape(sets: &[(&str, f64)]) -> FxInstance {
    let reg = kt_plugin_api::installed().expect("installed");
    let p = reg.get(kt_plugin_api::Family::Fx, "vhs_tape").expect("vhs_tape is built in");
    let m = p.manifest();
    let mut v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
    for (name, x) in sets {
        let i = m.passport.iter().position(|s| s.name == *name).expect("a row");
        v[i] = *x;
    }
    FxInstance::plugin_with("vhs_tape", &v).expect("vhs_tape")
}

/// |Δ| statistics, printed for the record of the measurement.
fn stats(cpu: &[u8], gpu: &[u8]) -> (u32, f64, f64) {
    let mut worst = 0;
    let (mut over, mut sum) = (0usize, 0u64);
    for (c, g) in cpu.iter().zip(gpu) {
        let d = (*c as i32 - *g as i32).unsigned_abs();
        worst = worst.max(d);
        sum += d as u64;
        if d > FrameTol::SIGNAL.max_lsb {
            over += 1;
        }
    }
    (worst, over as f64 / cpu.len() as f64, sum as f64 / cpu.len() as f64)
}

#[test]
fn t1186_the_tape_graph_is_within_its_tolerance_of_the_cpu() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let reg = kt_plugin_api::installed().expect("installed");
    let m = reg.get(kt_plugin_api::Family::Fx, "vhs_tape").expect("built in").manifest();
    let mut cases: Vec<(String, Vec<(&str, f64)>)> = vec![("defaults".into(), vec![]), ("ntsc".into(), vec![("standard", 1.0)])];
    for v in m.ui.variants {
        cases.push((v.name_key.to_string(), v.sets.to_vec()));
    }
    let grid2 = FxInstance::plugin_with("event_grid", &[2.0, 2.0, 0.0]).unwrap();
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let mut id = 7000;
    let mut worst_all = (0u32, 0f64, 0f64);
    for (name, sets) in &cases {
        for (form, alpha) in [("single", false), ("alpha", true), ("sandwich", false)] {
            let f = tape(sets);
            let fx = if form == "sandwich" { vec![grid2.clone(), f, zoom2.clone()] } else { vec![f] };
            id += 1;
            let src = Src { frame: frame(alpha), fx, id };
            let (cpu, gpu) = run(engine.as_ref(), &src, 90_000);
            let s = stats(&cpu, &gpu);
            eprintln!("t1186 {name} {form}: worst {}, over {:.4}%, mean {:.4}", s.0, s.1 * 100.0, s.2);
            worst_all = (worst_all.0.max(s.0), worst_all.1.max(s.1), worst_all.2.max(s.2));
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::SIGNAL, &format!("vhs_tape {name} {form}"));
            // deterministic on the device: the same plan twice
            let (_, again) = run(engine.as_ref(), &src, 90_000);
            assert!(again == gpu, "{name} {form}: the GPU gave two pictures for one plan");
        }
    }
    eprintln!("t1186 worst over all: |d| {}, over {:.4}%, mean {:.4}", worst_all.0, worst_all.1 * 100.0, worst_all.2);
}
