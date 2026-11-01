//! t1054 (plugin-api-barriers, D5; capability `plugins`): an inconsistent barrier is refused at
//! registration, by name, before any frame — and a well-formed one (including a graph of host
//! nodes alone, as blur's) is registered.

use std::sync::Arc;

use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, Registry, VideoClass};

/// What the test barrier's GPU graph looks like.
#[derive(Clone, Copy)]
enum Graph {
    /// a saved source, a half-size resample, a final pass reading both
    Good,
    /// a mip down and up, no pass of its own (blur)
    HostOnly,
    ReadsUnsaved,
    EndsHalfSize,
    /// a resample beyond the buffer ceiling at the largest K
    TooLarge,
    EndsOnSave,
    /// a pass naming a shader the kernel does not have
    NoSuchShader,
    Empty,
    /// a pass at a signal grid's own size, then a frame-size pass (vhs-tape-simulation, D6)
    GridGood,
    GridZero,
    GridTooLarge,
    /// the graph ends on the grid-size pass
    EndsOnGrid,
}

struct B {
    m: Manifest,
    kernel: bool,
    graph: Graph,
}

impl Plugin for B {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        if self.kernel { Some(self) } else { None }
    }
}

const PASS: &str = "@fragment\nfn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> { return textureLoad(T0, vec2<i32>(pos.xy), 0); }";

impl BarrierKernel for B {
    fn frame(&self, _k: &Cooked, f: Frame, _cx: &BarrierCtx, _s: &mut dyn Scratch) -> Frame {
        f
    }
    fn shaders(&self) -> &'static [&'static str] {
        &[PASS]
    }
    fn gpu(&self, _k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let pass = |inputs| Node::Pass { shader: 0, inputs, u: [0; 8], f: [0.0; 8] };
        let at = |w, h| Node::PassAt { shader: 0, inputs: [Slot::Prev, Slot::None, Slot::None], u: [0; 8], f: [0.0; 8], w, h };
        let half = Node::Resample { w: (g.w / 2).max(1), h: (g.h / 2).max(1) };
        match self.graph {
            Graph::Good => out.extend([Node::Save(0), half, pass([Slot::Saved(0), Slot::Prev, Slot::None])]),
            Graph::HostOnly => out.extend([half, Node::Resample { w: g.w, h: g.h }]),
            Graph::ReadsUnsaved => out.extend([half, pass([Slot::Saved(1), Slot::Prev, Slot::None])]),
            Graph::EndsHalfSize => out.extend([pass([Slot::Prev, Slot::None, Slot::None]), half]),
            Graph::TooLarge => out.extend([Node::Resample { w: g.w * 100, h: g.h }, Node::Resample { w: g.w, h: g.h }]),
            Graph::EndsOnSave => out.extend([pass([Slot::Prev, Slot::None, Slot::None]), Node::Save(0)]),
            Graph::NoSuchShader => out.push(Node::Pass { shader: 3, inputs: [Slot::Prev, Slot::None, Slot::None], u: [0; 8], f: [0.0; 8] }),
            Graph::Empty => {}
            Graph::GridGood => out.extend([at(1024, 576), pass([Slot::Prev, Slot::None, Slot::None])]),
            Graph::GridZero => out.extend([at(0, 576), pass([Slot::Prev, Slot::None, Slot::None])]),
            Graph::GridTooLarge => out.extend([at(5000, 576), pass([Slot::Prev, Slot::None, Slot::None])]),
            Graph::EndsOnGrid => out.extend([pass([Slot::Prev, Slot::None, Slot::None]), at(1024, 576)]),
        }
    }
}

fn barrier(id: &'static str, class: VideoClass, gpu: Gpu, kernel: bool, graph: Graph) -> Arc<dyn Plugin> {
    Arc::new(B {
        m: Manifest {
            id: id.into(),
            media: Media::VideoFx,
            passport: vec![ParamSpec::num("size", 1.0, 64.0, 4.0, 1.0, ParamUnit::Px, ParamKind::Smooth)],
            rows: Vec::new(),
            decl: Decl { video_class: Some(class), gpu, ..Decl::default() },
            ui: Default::default(),
        },
        kernel,
        graph,
    })
}

#[test]
fn t1054_an_inconsistent_barrier_is_refused_before_any_frame() {
    let mut reg = Registry::new();
    // well-formed: a pass graph, and a graph of host nodes alone
    reg.register(barrier("good", VideoClass::Barrier, Gpu::Barrier, true, Graph::Good)).expect("a good barrier");
    reg.register(barrier("host_only", VideoClass::Barrier, Gpu::Barrier, true, Graph::HostOnly))
        .expect("a graph of host nodes alone (blur) is legal");

    let cases: &[(&str, VideoClass, Gpu, bool, Graph, &str)] = &[
        ("no_kernel", VideoClass::Barrier, Gpu::Barrier, false, Graph::Good, "a barrier without a CPU kernel"),
        ("no_gpu", VideoClass::Barrier, Gpu::None, true, Graph::Good, "without a GPU kernel"),
        ("tap_text", VideoClass::Barrier, Gpu::Wgsl(PASS), true, Graph::Good, "a tap's WGSL instead of its pass graph"),
        ("on_pointwise", VideoClass::PointWise, Gpu::Barrier, true, Graph::Good, "on a plugin that is not a barrier"),
        ("unsaved", VideoClass::Barrier, Gpu::Barrier, true, Graph::ReadsUnsaved, "reads Saved(1) before it is saved"),
        ("half_size", VideoClass::Barrier, Gpu::Barrier, true, Graph::EndsHalfSize, "not at the frame size"),
        ("too_large", VideoClass::Barrier, Gpu::Barrier, true, Graph::TooLarge, "the ceiling is 4096"),
        ("ends_save", VideoClass::Barrier, Gpu::Barrier, true, Graph::EndsOnSave, "ends on a Save"),
        ("no_shader", VideoClass::Barrier, Gpu::Barrier, true, Graph::NoSuchShader, "names shader 3 of 1"),
        ("empty", VideoClass::Barrier, Gpu::Barrier, true, Graph::Empty, "empty on a frame with alpha"),
    ];
    for (id, class, gpu, kernel, graph, why) in cases {
        let err = reg.register(barrier(id, *class, *gpu, *kernel, *graph)).expect_err(id);
        assert_eq!(err.id, *id);
        assert!(err.reason.contains(why), "{id}: the refusal names {:?}, expected {why:?}", err.reason);
    }
    assert_eq!(reg.ids(), vec!["good", "host_only"], "a refused barrier left a trace in the registry");
}

/// t1187 (vhs-tape-simulation, D6; capability `plugins`): a pass at its own size is checked like a
/// resample — legal inside the graph, refused at zero or above the ceiling, and the graph must still
/// end at the frame size.
#[test]
fn t1187_a_pass_at_its_own_size_is_checked_like_a_resample() {
    let mut reg = Registry::new();
    reg.register(barrier("grid", VideoClass::Barrier, Gpu::Barrier, true, Graph::GridGood))
        .expect("a pass at the grid's size followed by a frame-size pass");
    let cases: &[(&str, Graph, &str)] = &[
        ("grid_zero", Graph::GridZero, "a pass at 0x576"),
        ("grid_large", Graph::GridTooLarge, "a pass at 5000x576 (the ceiling is 4096)"),
        ("ends_grid", Graph::EndsOnGrid, "ends at 1024x576, not at the frame size"),
    ];
    for (id, graph, why) in cases {
        let err = reg.register(barrier(id, VideoClass::Barrier, Gpu::Barrier, true, *graph)).expect_err(id);
        assert!(err.reason.contains(why), "{id}: the refusal names {:?}, expected {why:?}", err.reason);
    }
    assert_eq!(reg.ids(), vec!["grid"]);
}
