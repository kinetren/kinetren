//! t1052 (plugin-api-barriers 4.4; capability `plugins`): a barrier's pass graph runs on the GPU
//! executor assembled from the registry, within its tolerance class of the CPU.
//!
//! - A test barrier with a two-pass graph (a saved source, a mip down and up, a final pass reading
//!   both) that the host names nowhere runs alone, between two geometry series and under a layer
//!   strength below one — no host code knows it.
//! - Every registered barrier runs under a layer strength of 0.5 (t329 runs them alone and between
//!   geometry series).
//!
//! The uniform arena is consumed exactly as the graph predicts: the `debug_assert` in the
//! executor's `pass` holds in this debug build. A separate binary: its plugin set holds the test
//! plugin (the parity binary's does not).

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_time::Tick;

const W: u32 = 64;
const H: u32 = 36;

/// A barrier plugin the host names nowhere: the average of the source and its 2× mip, RGB, the
/// source's alpha. The GPU graph saves the source, mips it down and up, and its one pass reads both.
struct TwoPass {
    m: kt_plugin_api::Manifest,
}

const TWO_PASS_WGSL: &str = "@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let a = textureLoad(T0, p, 0);
    let b = textureLoad(T1, p, 0);
    return vec4<u32>((a.xyz + b.xyz + vec3<u32>(1u)) / 2u, a.w);
}
";

impl TwoPass {
    fn new() -> TwoPass {
        TwoPass {
            m: kt_plugin_api::Manifest {
                id: "two_pass".into(),
                media: kt_plugin_api::Media::VideoFx,
                passport: Vec::new(),
                rows: Vec::new(),
                decl: kt_plugin_api::Decl {
                    video_class: Some(kt_plugin_api::VideoClass::Barrier),
                    gpu: kt_plugin_api::Gpu::Barrier,
                    builtin: false,
                    ..kt_plugin_api::Decl::default()
                },
                ui: Default::default(),
            },
        }
    }
}

impl kt_plugin_api::Plugin for TwoPass {
    fn manifest(&self) -> &kt_plugin_api::Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

impl BarrierKernel for TwoPass {
    fn frame(&self, _k: &kt_plugin_api::Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let small = cx.raster.resample(&f, (f.w / 2).max(1), (f.h / 2).max(1), scratch);
        let up = cx.raster.resample(&small, f.w, f.h, scratch);
        scratch.put(small.rgba);
        let mut out = f;
        for (o, u) in out.rgba.chunks_exact_mut(4).zip(up.rgba.chunks_exact(4)) {
            for c in 0..3 {
                o[c] = ((o[c] as u32 + u[c] as u32 + 1) / 2) as u8;
            }
        }
        scratch.put(up.rgba);
        out
    }
    fn shaders(&self) -> &'static [&'static str] {
        &[TWO_PASS_WGSL]
    }
    fn gpu(&self, _k: &kt_plugin_api::Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        out.extend([
            Node::Save(0),
            Node::Resample { w: (g.w / 2).max(1), h: (g.h / 2).max(1) },
            Node::Resample { w: g.w, h: g.h },
            Node::Pass { shader: 0, inputs: [Slot::Saved(0), Slot::Prev, Slot::None], u: [0; 8], f: [0.0; 8] },
        ]);
    }
}

/// The plugin set of this binary: the built-ins plus `two_pass`, installed once.
fn install() {
    kt_plugins_builtin::install_with(|reg| {
        reg.register_for_test(Arc::new(TwoPass::new())).expect("two_pass registers");
    });
}

/// A deterministic frame: gradients + a green corner, optionally with a gradient alpha.
fn frame(alpha: bool) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let px = if x > W / 2 && y > H / 2 { [0, 255, 0] } else { [(x * 4) as u8, (y * 7) as u8, ((x + y) * 3) as u8] };
            rgba[i..i + 3].copy_from_slice(&px);
            rgba[i + 3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

/// One layer of `fx` over `frame` on both executors: (cpu, gpu).
fn run(engine: &dyn VideoGpuExec, id: u64, fx: Vec<FxInstance>, alpha: bool) -> (Vec<u8>, Vec<u8>) {
    let fr = frame(alpha);
    // the post-FX memo key must be unique per chain (the CPU path memoizes by it)
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
fn t1052_a_barrier_graph_runs_on_the_gpu_within_its_class() {
    install();
    let Some(engine) = test_engine() else { return };
    let half = FxInstance::LayerMix { amount: 0.5, from: 0 };
    let grid = FxInstance::plugin_with("event_grid", &[2.0, 2.0, 0.0]).unwrap();
    let zoom = FxInstance::geometry_zoom(2.0);
    let probe = FxInstance::plugin("two_pass").expect("the test plugin is installed");
    let mut id = 70_000;
    // the unknown two-pass barrier: alone, between two geometry series, under a strength of 0.5,
    // on opaque and alpha frames
    for (what, chain) in [
        ("alone", vec![probe.clone()]),
        ("between geometry", vec![grid.clone(), probe.clone(), zoom.clone()]),
        ("under strength", vec![probe.clone(), half.clone()]),
    ] {
        for alpha in [false, true] {
            id += 1;
            let (cpu, gpu) = run(engine.as_ref(), id, chain.clone(), alpha);
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::BILINEAR, &format!("two_pass {what} alpha={alpha}"));
        }
    }
    // every registered barrier under a strength of 0.5: the sandwich class of t329
    let threshold = ["greenscreen", "edges", "noise", "halftone", "camera_isp"];
    let reg = kt_plugin_api::installed().expect("installed");
    let mut kinds = 0;
    for p in reg.plugins().filter(|p| p.barrier().is_some() && p.manifest().id != "two_pass") {
        let f = FxInstance::plugin(&p.manifest().id).expect("a registered kind");
        let tol = if threshold.contains(&f.tag()) { FrameTol::THRESHOLD } else { FrameTol::BILINEAR };
        for alpha in [false, true] {
            id += 1;
            let (cpu, gpu) = run(engine.as_ref(), id, vec![f.clone(), half.clone()], alpha);
            assert_frames_close(&cpu, &gpu, W, H, tol, &format!("{} under strength alpha={alpha}", f.tag()));
        }
        kinds += 1;
    }
    assert!(kinds >= 12, "the registry holds {kinds} barrier kinds, the twelve moved ones at least");
}
