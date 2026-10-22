//! Frame plan (R.29): the STRUCTURE of the composition at moment t, built before
//! the choice of backend. The Vegas order, dissolve pairs, solo fades, levels —
//! are decided once in `VideoRenderer` (video.rs, build_plan); there are
//! two executors: CPU (`compose_from_cache*` — the byte reference and fallback, invariant No. 11)
//! and GPU (crate kt-gpu, wgpu f32 — injected from above, kt-engine does not
//! depend on it). “One rendering path” (invariant No. 7) = one plan builder;
//! the backend is an execution parameter, not a second path.

use std::borrow::Cow;
use std::sync::Arc;

use kt_cache::SourceId;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

/// The layer's resolved 3D camera: since R.44 it lives in kt-fx (`kt_fx::geom::Cam3d`)
/// — Pan3d is computed by BOTH backends, and the shared homography math
/// sits alongside the rest of the shared parametrics (`pan_frame`/`cam_at`).
pub use kt_fx::geom::Cam3d;

/// The source of one layer: the pre-FX frame (already read from the cache when the
/// plan is built — recency is ALWAYS bumped, the coverage bars do not “cool down”) +
/// the clip's FX chain.
pub struct LayerSrc<'a> {
    pub source: SourceId,
    /// Index of the pre-FX frame in the cache (source pts in ticks).
    pub cache_idx: i64,
    /// The CONTENT SERIAL NUMBER of the frame (R.53, item 409). The addresses
    /// `(source, cache_idx, size)` are not enough: for a generator with new
    /// parameters, and for a file that came back in place of a placeholder, the address is THE SAME,
    /// while the bytes differ — by address the GPU would return the old texture and the
    /// old memoized composite (t496, t211).
    pub content: u64,
    /// The layer's CHAIN INPUT (R.46, item 318; refined in R.58, item 459): the frame
    /// that the chain receives is `src_scale` times larger than Quality. 1 —
    /// exactly Quality, as it always was. For a LEAF this is the decode scale step;
    /// for a REEL it is `min(the reel's step, how much this instance's chain asks for)`:
    /// the reel's step is one for all its instances, but
    /// not every chain can consume it (`NestSrc::fit_to`).
    pub src_scale: u32,
    /// Layer input: a ready pre-FX frame or a SUBTREE (R.50).
    pub input: LayerInput<'a>,
    /// The chain WITH automation already applied (R.30): curves are resolved
    /// once when the plan is built — both executors see the ready
    /// parameters. Without curves — a borrow from VideoClip (zero clones).
    pub fx: Cow<'a, [FxInstance]>,
    /// Post-FX memoization key of the CPU path.
    pub fx_hash: u64,
    /// Clip progress 0..1 (CamMotion).
    pub u: f64,
    /// Some — the chain has a 3D effect (the camera is resolved by t). Since R.44
    /// Pan3d is computed by BOTH backends (inverse homography in the fused pass);
    /// None when there is 3D in the chain — the effect is skipped, as before without a camera.
    pub cam3d: Option<Cam3d>,
    /// Frame blending (R.37 C5, the mode “Frames: blending”): WHAT and with what
    /// weight `frame` is blended. `None` — “nearest” or there is no warp,
    /// the path stays byte-for-byte as before.
    ///
    /// The BLEND itself is done HERE, in the plan's common code, not in the executors:
    /// invariant No. 7 — “the composition code is one, the backend is an execution
    /// parameter”. Otherwise the same arithmetic would have to be kept
    /// in sync in the CPU kernel and in WGSL, and that is exactly the class of divergence
    /// on which preview and export show different things.
    ///
    /// The field lives in the layer, NOT as a separate `PlanLayer` branch: blending is a
    /// property of the source of ONE clip, and such a clip may stand under
    /// any of the branches, including both halves of a `Dissolve`.
    pub blend: Option<LayerBlend>,
    /// BLEND MODE of this layer onto the layers BELOW (R.62, change B).
    /// `Normal` — the previous `alpha_over` byte for byte, and GOLDEN_VIDEO rests on it;
    /// the others are computed by the `blend_over` kernel (CPU) and its
    /// WGSL mirror. The field lives in the layer, not as a separate
    /// `PlanLayer` branch: overlay is a property of ANY layer, not of a special kind.
    pub blend_mode: kt_model::BlendMode,
    /// Context of the leading “Geometry” node (R.62): the source kind and the
    /// allowed conform rule. Both executors pass it to
    /// the chain planner — the frame is computed by one code.
    pub lead: Option<kt_fx::conform::LeadCtx>,
}

/// Layer input: a frame or a sub-composite of a nested reel (R.50, wave B).
///
/// A nesting does NOT get a `PlanLayer` branch of its own, on purpose: it is a clip, it has
/// a level, a solo fade, `u`, a camera, and it may be a half of a
/// `Dissolve`. A separate branch would duplicate the logic of level and fades in
/// BOTH executors — exactly what invariant 7 is against. Only the
/// SOURCE of the layer's pre-FX frame changes.
pub enum LayerInput<'a> {
    /// Pre-FX frame; None — starvation. The executors decide for themselves: the CPU may
    /// return a memoized post-FX frame even without it, the GPU cannot.
    Frame(Option<Arc<VideoFrame>>),
    /// Subtree: the reel's plan, assembled on the instance's INNER time.
    Nest(Box<NestSrc<'a>>),
}

impl<'a> LayerInput<'a> {
    /// A ready frame, if the input is a frame. For a subtree it is always `None`:
    /// it still has to be executed.
    pub fn frame(&self) -> Option<&Arc<VideoFrame>> {
        match self {
            LayerInput::Frame(f) => f.as_ref(),
            LayerInput::Nest(_) => None,
        }
    }

    pub fn nest(&self) -> Option<&NestSrc<'a>> {
        match self {
            LayerInput::Nest(n) => Some(n),
            LayerInput::Frame(_) => None,
        }
    }
}

/// Subtree of the nesting.
pub struct NestSrc<'a> {
    /// The reel's address — it is also the `SourceId` of the sub-composite in the cache.
    pub seq: kt_model::MediaId,
    /// The hash of the reel's REVISION: an edit inside touches no field of the
    /// instance, and without it the memo would return stale data.
    pub content_hash: u64,
    /// The inner time at which the plan was assembled (the memoization key).
    /// CONTINUOUS (R.56, item 448): a reel has no frames to
    /// snap to, and rounding turned the animation inside a slowed
    /// composition into two-frame steps.
    pub inner_t: Tick,
    /// Frame blending is inherited onto the content (R.56, item 448): the leaves
    /// inside read the PAIR. The flag is part of the memoization key (`nest_memo_key`)
    /// and of the GPU plan key: the same reel and time in two modes give different
    /// bytes.
    pub blend: bool,
    /// The reel's STEP (R.50 B5): `plan.out_w == nest_scale × Quality`
    /// of the parent. It is ONE per reel (the maximum over all instances,
    /// `nest_scales`) — otherwise the inner leaf would have two cache keys,
    /// two coverage bars and two orders (t450, t312/t317).
    ///
    /// The CHAIN INPUT of this instance is a different number (`LayerSrc.src_scale`):
    /// a chain without resample (empty, a single layer strength, a leading stop, only
    /// barriers) does not consume the step, and an S× frame would never be returned to
    /// Quality. If they diverge, the executor FITS the sub-composite before the
    /// chain (`fit_to`), R.58 item 459.
    pub nest_scale: u32,
    pub plan: FramePlan<'a>,
}

impl NestSrc<'_> {
    /// Where to fit the sub-composite BEFORE the layer's chain. `None` — the reel's step
    /// equals the chain input, there is no step and the bytes are as before.
    ///
    /// The decision is ONE for both backends (invariant 7): the CPU calls `fit_into`,
    /// the GPU places a `Resample` to the full frame — its mirror.
    pub fn fit_to(&self, src_scale: u32, out_w: u32, out_h: u32) -> Option<(u32, u32)> {
        if self.nest_scale == src_scale {
            return None;
        }
        debug_assert!(
            src_scale < self.nest_scale,
            "chain input {src_scale}x is above the reel stage {}x",
            self.nest_scale
        );
        // the sub-plan is assembled in the reel's BASE × step: the base is the parent's canvas
        // or, for a reel with its own raster (R.62), the conform rule's frame
        let s = self.nest_scale.max(1);
        let (bw, bh) = (self.plan.out_w / s, self.plan.out_h / s);
        let _ = (out_w, out_h);
        Some((bw * src_scale, bh * src_scale))
    }
}

/// What and how the layer's frame is blended with: the neighbour (for cache keys) and the weight in 4096ths.
#[derive(Debug, Clone, Copy)]
pub struct LayerBlend {
    pub cache_idx: i64,
    pub k: u32,
    /// The content serial number of the NEIGHBOUR (item 409): the blended frame is a
    /// function of both, so both go into the key.
    pub content: u64,
}

/// A solo fade WITH a KIND (R.33 C4): the second side is transparency, so
/// there is one frame, and the shape is given by the same transition kernel. `incoming` — the clip
/// APPEARS (a = emptiness, b = frame); otherwise it leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoloFade {
    pub k: u32,
    pub mix: crate::TransMix,
    pub incoming: bool,
}

#[allow(clippy::large_enum_variant)]
pub enum PlanLayer<'a> {
    /// An ordinary layer or a solo fade. `alpha_k` (0..4096) — level × the fade
    /// weight; `scale_alpha` — run through alpha scaling even
    /// at 4096 (v1 semantics of the solo fade: a frame without alpha gets A=255,
    /// not a copy of the raw alpha bytes). `fade` — a solo fade that is NOT a crossfade
    /// (R.33): the weight then is not in alpha_k, but in the kernel.
    One { src: LayerSrc<'a>, alpha_k: u32, scale_alpha: bool, fade: Option<SoloFade> },
    /// Transition pair: for a crossfade dst = a·(1−k) + b·k (k out of 4096
    /// parts), otherwise the shape is given by `mix`; then alpha × alpha_k (the levels
    /// of the neighbours are already blended with the same weight at build time).
    Dissolve { a: LayerSrc<'a>, b: LayerSrc<'a>, k: u32, alpha_k: u32, mix: crate::TransMix },
}

/// Composition plan of the frame at moment t: layers back-to-front (the Vegas order
/// is already applied), size — the caller's Quality.
pub struct FramePlan<'a> {
    pub t: Tick,
    pub out_w: u32,
    pub out_h: u32,
    pub layers: Vec<PlanLayer<'a>>,
    /// Memoize post-FX frames in kt-cache (preview only). In export
    /// each t is composited EXACTLY once — memoization gives no hits there,
    /// but full-size post-FX frames were evicting the not-yet-used
    /// pre-frames of neighbouring layers from the export cache (R.31 item 4: “the render
    /// is broken” — missing layers on a multi-layer edit with FX).
    pub memoize_postfx: bool,
}

/// GPU executor error. `DeviceLost` — the caller “poisons” the GPU until the end
/// of the session (CPU fallback + notice); `TooLarge` — a fallback only for
/// this plan (the source is larger than the device's texture limit).
#[derive(Debug)]
pub enum GpuExecError {
    DeviceLost,
    Validation(String),
    TooLarge {
        w: u32,
        h: u32,
    },
    /// The GPU executor cannot yet handle a plan of this kind — a fallback to the CPU
    /// ONLY for this plan, like `TooLarge`. Not “poisoning”: the device
    /// is alive, and the next frame may go to the GPU again.
    Unsupported(&'static str),
}

/// Backend passport (notice and perf-panel line).
#[derive(Debug, Clone)]
pub struct GpuBackendInfo {
    pub name: String,
    pub backend: String,
    /// A software rasterizer (llvmpipe/lavapipe) is correct and counts as “GPU
    /// present” (R.29); the flag is for diagnostics only.
    pub software: bool,
}

/// The GPU executor of the plan (implementation — the kt-gpu crate; kt-engine knows
/// only the trait and receives the executor by injection from above).
pub trait VideoGpuExec: Send + Sync {
    /// Plan → straight RGBA8 into `out` (readback). Ok(complete) — the same
    /// “playback does not wait” rules (invariant No. 5) as the CPU executor;
    /// there is one difference: the GPU does not see the memoized post-FX frame of the CPU path,
    /// so a layer without a pre-FX frame is always starvation for it.
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError>;
    fn info(&self) -> &GpuBackendInfo;
}
