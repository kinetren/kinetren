//! The barrier contract (plugin-api-barriers, D1–D3): a video effect that needs the neighbourhood
//! of a pixel — blur, glow, edges, a mosaic — runs as a pass of its own between geometry series.
//!
//! A barrier plugin gives two things, both pure:
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

/// The counter-based hash of the grain (lowbias32; kt-fx `noise_hash` re-exports it): integer and
/// stateless.
pub use crate::generator::lowbias32 as noise_hash;

/// The frame's grain seed from its burst index (noise, vhs; kt-fx `noise_seed` re-exports it).
#[inline]
pub fn noise_seed(burst_index: i64) -> u32 {
    noise_hash(burst_index as u32)
}

/// A frame on the CPU side of the host (a generator's or a stream stage's input): straight RGBA8,
/// `w×h`. kt-fx re-exports it.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
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
    /// The same pass rendered at `w×h` instead of the frame size (vhs-tape-simulation, D6): a
    /// signal grid of its own resolution (the VHS standard's 4 fsc grid). Its output is the next
    /// `Prev` at that size; the graph must still end at the frame size.
    PassAt { shader: u8, inputs: [Slot; 3], u: [u32; 8], f: [f32; 8], w: u32, h: u32 },
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
    /// The plugin's fragment passes (WGSL with `fn fs`, over the host's `common.wgsl`); empty — a
    /// barrier without a GPU kernel (refused for a built-in).
    fn shaders(&self) -> &'static [&'static str] {
        &[]
    }
    /// The GPU graph for this frame; an empty graph is a passthrough (drop_shadow on an opaque frame).
    fn gpu(&self, _k: &Cooked, _g: &GpuCtx, _out: &mut Vec<Node>) {}
    /// The GPU graph from the passport values; the default cooks them (as `frame_values`).
    fn gpu_values(&self, values: &[f64], g: &GpuCtx, out: &mut Vec<Node>) {
        self.gpu(&self.cook(values), g, out)
    }
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
            Node::Pass { shader, inputs, .. } | Node::PassAt { shader, inputs, .. } => {
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
                size = match n {
                    Node::PassAt { w, h, .. } => {
                        if *w == 0 || *h == 0 || *w > max_dim || *h > max_dim {
                            return Err(format!("a pass at {w}x{h} (the ceiling is {max_dim})"));
                        }
                        (*w, *h)
                    }
                    _ => (g.w, g.h),
                };
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
