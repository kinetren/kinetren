//! t1210 (camera-isp-simulation, D2; capability `plugins`): a barrier sees ALL its rows through the
//! values door — the CPU executor, the GPU graph and the registry's graph check call
//! `frame_values`/`gpu_values`, not the eight cooked numbers.

use kt_fx::video::{apply_barrier, Frame, FxScratch, VideoFxCtx};
use kt_model::FxInstance;
use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, Registry, VideoClass};

/// A twelve-row barrier whose output depends on row 11 only: the red channel becomes that row. With
/// `bad`, its graph ends at half the frame size when row 11 is at its top (a graph the registry must
/// refuse — it can only see that through the values door).
struct Wide {
    m: Manifest,
    bad: bool,
}

const SHADER: &str = "@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {\n\
    let c = textureLoad(T0, vec2<i32>(floor(pos.xy)), 0);\n\
    return vec4<u32>(P.u0.x, c.g, c.b, c.a);\n}\n";

impl Wide {
    fn new(id: &'static str, bad: bool) -> Wide {
        const NAMES: [&str; 12] = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11"];
        let passport =
            NAMES.iter().map(|n| ParamSpec::num(n, 0.0, 255.0, 10.0, 1.0, ParamUnit::None, ParamKind::Smooth)).collect();
        Wide {
            m: Manifest {
                id: id.into(),
                media: Media::VideoFx,
                passport,
                rows: Vec::new(),
                decl: Decl { video_class: Some(VideoClass::Barrier), gpu: Gpu::Barrier, builtin: false, ..Decl::default() },
                ui: Default::default(),
            },
            bad,
        }
    }
}

impl Plugin for Wide {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

impl BarrierKernel for Wide {
    fn frame(&self, _k: &Cooked, _f: Frame, _cx: &BarrierCtx, _s: &mut dyn Scratch) -> Frame {
        panic!("the host called the cooked form of a barrier that has a values door");
    }
    fn frame_values(&self, p: &[f64], mut f: Frame, _cx: &BarrierCtx, _s: &mut dyn Scratch) -> Frame {
        for px in f.rgba.chunks_exact_mut(4) {
            px[0] = p[11] as u8;
        }
        f
    }
    fn shaders(&self) -> &'static [&'static str] {
        &[SHADER]
    }
    fn gpu_values(&self, p: &[f64], g: &GpuCtx, out: &mut Vec<Node>) {
        out.push(Node::Pass { shader: 0, inputs: [Slot::Src, Slot::None, Slot::None], u: [p[11] as u32, 0, 0, 0, 0, 0, 0, 0], f: [0.0; 8] });
        if self.bad && p[11] >= 255.0 {
            out.push(Node::Resample { w: g.w / 2, h: g.h / 2 });
        }
    }
}

fn install() {
    kt_plugins_builtin::install_with(|reg| {
        reg.register_for_test(std::sync::Arc::new(Wide::new("wide_probe", false))).expect("wide_probe registers");
    });
}

fn frame() -> Frame {
    Frame { w: 20, h: 10, rgba: vec![100; 20 * 10 * 4], has_alpha: false }
}

fn ctx() -> VideoFxCtx {
    VideoFxCtx { out_w: 20, out_h: 10, u: 0.5, burst_index: 90_000, cam3d: None, in_scale: 1, lead: None }
}

#[test]
fn t1210_a_row_past_the_eighth_reaches_the_kernel() {
    install();
    // the CPU executor: row 11 at two values gives two outputs
    let run = |v11: f64| {
        let mut vals = vec![10.0; 12];
        vals[11] = v11;
        let f = FxInstance::plugin_with("wide_probe", &vals).expect("the test plugin is installed");
        apply_barrier(&f, frame(), &ctx(), &mut FxScratch::default(), 1).rgba
    };
    let (a, b) = (run(40.0), run(200.0));
    assert_eq!(a[0], 40, "row 11 did not reach the CPU kernel");
    assert_eq!(b[0], 200, "row 11 did not reach the CPU kernel");

    // the GPU graph: built from the values
    let reg = kt_plugin_api::installed().expect("installed");
    let p = reg.get(kt_plugin_api::Family::Fx, "wide_probe").expect("registered");
    let g = GpuCtx { w: 20, h: 10, res_scale: 1, alpha: Some(false), src_has_alpha: false, burst_index: 0 };
    let graph = |v11: f64| {
        let mut vals = vec![10.0; 12];
        vals[11] = v11;
        let mut out = Vec::new();
        p.barrier().expect("a barrier").gpu_values(&vals, &g, &mut out);
        out
    };
    assert_ne!(graph(40.0), graph(200.0), "row 11 did not reach the GPU graph");

    // the registry checks the graph on the passport's top THROUGH the values door
    let mut fresh = Registry::new();
    let refused = fresh.register(std::sync::Arc::new(Wide::new("wide_bad", true)));
    assert!(refused.is_err(), "a graph broken only at row 11's top was accepted: the check read the cooked form");
    assert!(fresh.get(kt_plugin_api::Family::Fx, "wide_bad").is_none(), "a refused plugin entered the registry");
}

/// The cooked barriers are untouched by the door: blur's default through the values door equals its
/// cooked form byte for byte (its frozen golden is t1051).
#[test]
fn t1210_a_cooked_barrier_keeps_its_bytes_through_the_door() {
    install();
    let reg = kt_plugin_api::installed().expect("installed");
    let blur = reg.get(kt_plugin_api::Family::Fx, "blur").expect("blur");
    let b = blur.barrier().expect("a barrier");
    let vals = kt_plugin_api::ParamValues::defaults(&blur.manifest().passport).0;
    let cx = BarrierCtx { res_scale: 1, burst_index: 0, simd: false, raster: &kt_fx::video::Raster };
    let via_door = b.frame_values(&vals, frame(), &cx, &mut FxScratch::default()).rgba;
    let cooked = b.frame(&b.cook(&vals), frame(), &cx, &mut FxScratch::default()).rgba;
    assert_eq!(via_door, cooked);
}
