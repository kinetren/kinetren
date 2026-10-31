//! The barrier contract (plugin-api-barriers, D1–D3): a video effect that needs the neighbourhood
//! of a pixel — blur, glow, edges, a mosaic — runs as a pass of its own between geometry series.
//!
//! A barrier plugin gives three things, all pure:
//! - the CPU kernel over a whole frame (`BarrierKernel::frame`): the BYTE REFERENCE. When the host
//!   says SIMD is allowed (`BarrierCtx::simd` — the ONE switch, `KT_SIMD` and CPU detection, decided
//!   by the host) the kernel may take its own AVX2 path, which MUST give the same bytes (t1053);
//! - the GPU pass graph (`BarrierKernel::gpu`): the host's nodes (resample, premultiplication,
//!   save) and the plugin's fragment passes (`BarrierKernel::shaders`, each compiled ONCE per
//!   executor over the host's `common.wgsl`), with its uniforms computed here in f64 and rounded to
//!   f32 once;
//! - its relation to alpha (`Decl.alpha`).
//!
//! The K× rule (`res_scale`) is the host's: the planner decides K and hands it over; the kernel only
//! scales its pixel geometry by it (the look is the same, at K=1 the former code).

use crate::Cooked;

/// The largest K the planner hands a barrier (kt-fx `plan_chain`: the quantum of the zoom, ceiling 4).
pub const MAX_RES_SCALE: u32 = 4;
/// The ceiling of an intermediate buffer's side — a project constant, not a device limit (kt-fx
/// `MAX_INTER_DIM`, the same number).
pub const MAX_INTER_DIM: u32 = 4096;

/// A frame in progress: straight RGBA8, `w×h`. The host's frame type (kt-fx re-exports it), so a
/// kernel hands the frame back without a copy.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
}

/// The host's buffer pool (kt-fx `FxScratch`): a barrier takes its output buffer here and gives the
/// input's back, so a frame path does not allocate (invariant 12).
pub trait Scratch {
    /// A zeroed buffer of `len` bytes.
    fn take(&mut self, len: usize) -> Vec<u8>;
    /// Return a buffer to the pool.
    fn put(&mut self, b: Vec<u8>);
}

/// The raster code the host shares with its own paths (D3): ONE text each, with the host's SIMD
/// twin where it has one. A plugin reaches it through the context and never copies it.
pub trait RasterHost: Sync {
    /// The host's bilinear resample of the WHOLE frame to `tw×th` (kt-fx `bilinear_into`, its AVX2
    /// twin when the host allows it — the mirror of the GPU `Resample` node): the mip of glow,
    /// sharpen and drop_shadow. The alpha flag travels with the frame.
    fn resample(&self, src: &Frame, tw: u32, th: u32, scratch: &mut dyn Scratch) -> Frame;
    /// A bilinear sample of straight RGBA in f64, at the coordinates of kt-fx `sample_bilinear_f64`
    /// (source pixels, taps clamped to the edges).
    fn sample(&self, f: &Frame, x: f64, y: f64) -> [f64; 4];
    /// Premultiplied alpha in place, and its inverse (kt-fx `premultiply`/`unpremultiply`).
    fn premultiply(&self, rgba: &mut [u8]);
    fn unpremultiply(&self, rgba: &mut [u8]);
}

/// What a barrier sees of the host for one frame.
pub struct BarrierCtx<'a> {
    /// K: the frame is K× the chain's logical size; the kernel scales its pixel geometry by it.
    pub res_scale: u32,
    /// The absolute timeline position in ticks — the grain of noise and vhs is pinned to it.
    pub burst_index: i64,
    /// The host allows the kernel's SIMD path (the one switch; R.32).
    pub simd: bool,
    pub raster: &'a dyn RasterHost,
}

/// How a step relates to alpha (R.53, item 417): exactly one, declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlphaRel {
    /// The alpha of the input passes through (it stays silent in the fold).
    #[default]
    Preserves,
    /// It may CREATE transparency (greenscreen by key).
    Produces,
    /// It guarantees an opaque output.
    Destroys,
}

/// What a barrier's GPU graph is built against: the frame at this point of the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuCtx {
    pub w: u32,
    pub h: u32,
    pub res_scale: u32,
    /// The alpha of the frame at this point as the chain's fold knows it; `None` — the fold is
    /// silent, so it is the source's (`src_has_alpha`).
    pub alpha: Option<bool>,
    pub src_has_alpha: bool,
    pub burst_index: i64,
}

impl GpuCtx {
    /// Whether the frame here has alpha: the fold's word, or the source's when the fold is silent.
    pub fn has_alpha(&self) -> bool {
        self.alpha.unwrap_or(self.src_has_alpha)
    }
}

/// A texture a pass reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The barrier's input.
    Src,
    /// The previous node's output.
    Prev,
    /// The output a `Save(n)` kept.
    Saved(u8),
    /// Nothing (a 1×1 stub).
    None,
}

/// One node of a barrier's GPU graph. The graph starts with `Prev = Src`; each node but `Save`
/// produces the next `Prev`; the barrier's output is the last `Prev`, at the frame size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Node {
    /// Keep the current `Prev` as `Saved(n)` (no pass).
    Save(u8),
    /// The host's premultiplication of `Prev` (R.53, item 418).
    Premul,
    Unpremul,
    /// The host's bilinear resample of `Prev` to `w×h` (the mirror of `RasterHost::resample`).
    Resample { w: u32, h: u32 },
    /// The plugin's fragment pass `shaders()[shader]` at the frame size, reading up to three slots
    /// as `T0..T2`; `u`/`f` are the pass's `P.u0..u1`/`P.f0..f1`.
    Pass { shader: u8, inputs: [Slot; 3], u: [u32; 8], f: [f32; 8] },
}

/// The kernel of a barrier plugin.
pub trait BarrierKernel: Send + Sync {
    /// The parameters prepared ONCE per frame (clamped, cast); the default — the values themselves.
    fn cook(&self, p: &[f64]) -> Cooked {
        let mut k = [0.0; 8];
        let n = p.len().min(8);
        k[..n].copy_from_slice(&p[..n]);
        k
    }
    /// The whole frame on the CPU: the byte reference (its SIMD path only when `cx.simd`).
    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame;
    /// The plugin's fragment passes (WGSL with `fn fs`, over the host's `common.wgsl`); empty — a
    /// barrier without a GPU kernel (refused for a built-in).
    fn shaders(&self) -> &'static [&'static str] {
        &[]
    }
    /// The GPU graph for this frame; an empty graph is a passthrough (drop_shadow on an opaque frame).
    fn gpu(&self, _k: &Cooked, _g: &GpuCtx, _out: &mut Vec<Node>) {}
}

/// Check a graph's shape (D5): every `Saved(n)` read after its `Save(n)`, every pass a shader the
/// kernel has, no resample side of zero or above `max_dim`, no `Save` last, and the output at the frame size (a
/// graph of host nodes alone is legal — blur is a mip down and up). The registry runs it on the
/// defaults and the top of the passport; the host in debug on every frame.
pub fn check_graph(nodes: &[Node], shaders: usize, g: &GpuCtx, max_dim: u32) -> Result<(), String> {
    let mut saved = [false; 256];
    let mut size = (g.w, g.h);
    for n in nodes {
        match n {
            Node::Save(i) => saved[*i as usize] = true,
            Node::Premul | Node::Unpremul => {}
            Node::Resample { w, h } => {
                if *w == 0 || *h == 0 || *w > max_dim || *h > max_dim {
                    return Err(format!("a resample to {w}x{h} (the ceiling is {max_dim})"));
                }
                size = (*w, *h);
            }
            Node::Pass { shader, inputs, .. } => {
                if *shader as usize >= shaders {
                    return Err(format!("a pass names shader {shader} of {shaders}"));
                }
                for s in inputs {
                    if let Slot::Saved(i) = s
                        && !saved[*i as usize]
                    {
                        return Err(format!("a pass reads Saved({i}) before it is saved"));
                    }
                }
                size = (g.w, g.h);
            }
        }
    }
    // a graph ending on `Save` would hand its last texture to the saved list, which the host
    // releases while the output still reads it
    if matches!(nodes.last(), Some(Node::Save(_))) {
        return Err("the graph ends on a Save".into());
    }
    if size != (g.w, g.h) {
        return Err(format!("the graph ends at {}x{}, not at the frame size {}x{}", size.0, size.1, g.w, g.h));
    }
    Ok(())
}
