//! The GPU executor of FramePlan: fragment passes (NOT compute: the default
//! egui-wgpu device on the GL backend lives within the downlevel limits, and the live check
//! runs on llvmpipe), ping-pong composite, readback of the final frame.
//! The semantics mirror the CPU executor of kt-engine (execute_plan_cpu):
//! integer kernels byte for byte, bilinear ones within tolerance (t181/t182).

use std::sync::{Arc, Mutex};

use kt_engine::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, TransMix, VideoGpuExec,
    video_plan::SoloFade,
};
use kt_model::FxInstance;
use kt_time::Tick;
use xxhash_rust::xxh3::Xxh3;

use crate::context::{GpuContext, GpuInitError};
use crate::pool::{FORMAT, ReadbackCache, Tex, TexturePool, UploadCache, create_tex};

const SLOT: u64 = 256; // dynamic offset alignment (the maximum per the spec)
/// Uniform of the merged geometry pass (R.44): the primitive program does not fit
/// into the shared 128 B, so it has its own bind group layout and 4 arena slots per pass.
const GEOM_UNIFORM_BYTES: usize = 1024;
const GEOM_SLOT_UNITS: u32 = (GEOM_UNIFORM_BYTES as u64 / SLOT) as u32;
const UPLOAD_BUDGET: u64 = 256 * 1024 * 1024;
const TRANSIENT_BUDGET: u64 = 128 * 1024 * 1024;
const READBACK_BUDGET: u64 = 64 * 1024 * 1024;

/// Uniform of all passes: 2×vec4<u32> + 2×vec4<f32> + mat4 (see common.wgsl).
#[derive(Default, Clone, Copy)]
struct UniformData {
    u: [u32; 8],
    f: [f32; 8],
    /// MVP of the 3D passes (column-major, as WGSL mat4x4 expects).
    m: [f32; 16],
}

impl UniformData {
    fn bytes(&self) -> [u8; 128] {
        let mut b = [0u8; 128];
        for (i, v) in self.u.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.f.iter().enumerate() {
            b[32 + i * 4..32 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.m.iter().enumerate() {
            b[64 + i * 4..64 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

/// Uniform of geom_fused.wgsl (a mirror of the GeomParams structure): a header plus
/// vec4 primitive slots. 1024 B = GEOM_SLOT_UNITS arena slots.
struct GeomUniform {
    h: [u32; 4],
    d: [u32; 4],
    p: [[f32; 4]; 62],
}

impl GeomUniform {
    fn bytes(&self) -> [u8; GEOM_UNIFORM_BYTES] {
        let mut b = [0u8; GEOM_UNIFORM_BYTES];
        for (i, v) in self.h.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.d.iter().enumerate() {
            b[16 + i * 4..16 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (s, sl) in self.p.iter().enumerate() {
            for (c, v) in sl.iter().enumerate() {
                let o = 32 + s * 16 + c * 4;
                b[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        b
    }
}

/// Packing of the kt-fx program into the uniform: the coefficients are already multiplied in f64
/// at construction; here is a ONE-TIME rounding f64→f32 of each number.
fn encode_geom(prog: &kt_fx::geom::GeomProgram) -> GeomUniform {
    use kt_fx::geom::GeomPrim;
    let mut u = GeomUniform { h: [0; 4], d: [0; 4], p: [[0.0; 4]; 62] };
    let mut kinds: u64 = 0;
    let mut cur = 0usize;
    debug_assert!(prog.prims.len() <= kt_fx::geom::MAX_PRIMS);
    for (i, p) in prog.prims.iter().enumerate() {
        let kind: u64 = match p {
            GeomPrim::Affine(a) => {
                u.p[cur] = [a.ax as f32, a.bx as f32, a.ay as f32, a.by as f32];
                cur += 1;
                0
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                u.p[cur] = [*x0 as f32, *x1 as f32, *y0 as f32, *y1 as f32];
                cur += 1;
                1
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                u.p[cur] = [*gw as f32, *gh as f32, *cw as f32, *ch as f32];
                u.p[cur + 1] = [*in_w as f32, *in_h as f32, 0.0, 0.0];
                cur += 2;
                2
            }
            GeomPrim::Proj { m } => {
                for (r, row) in m.iter().enumerate() {
                    u.p[cur + r] = [row[0] as f32, row[1] as f32, row[2] as f32, 0.0];
                }
                cur += 3;
                3
            }
            GeomPrim::Warp { warp, p } => {
                u.p[cur] = [plugin_warp_mode(warp.id) as f32, p[0] as f32, p[1] as f32, p[2] as f32];
                u.p[cur + 1] = [p[3] as f32, p[4] as f32, p[5] as f32, p[6] as f32];
                cur += 2;
                4
            }
            GeomPrim::Bounds { x0, x1, y0, y1 } => {
                u.p[cur] = [*x0 as f32, *x1 as f32, *y0 as f32, *y1 as f32];
                cur += 1;
                5
            }
        };
        kinds |= kind << (i * 4);
    }
    debug_assert!(cur <= 46, "geom prims exceed uniform slots (tap table at 46+)");
    // tap table of the absorbed pointwise ops (R.45, item 312): op i has slots
    // 46+2i; meta = kind (4 bits, R.47) | after_prim<<4 | color<<9 | special<<10
    debug_assert!(prog.tap_ops.len() <= kt_fx::geom::MAX_TAP_OPS);
    for (i, op) in prog.tap_ops.iter().enumerate() {
        let base = 46 + i * 2;
        match op.kind {
            kt_fx::geom::TapKind::Plugin(t) => {
                let kind = plugin_tap_kind(t.id);
                let meta = kind | ((op.after_prim as u32) << 4);
                let g = t.gpu();
                u.p[base] = [f32::from_bits(meta), g[0], g[1], g[2]];
                u.p[base + 1] = [g[3], g[4], g[5], g[6]];
                if t.positioned {
                    // the tap's stage size (D11.1) — the host's half of `b`
                    u.p[base + 1][2] = t.stage.stage_w as f32;
                    u.p[base + 1][3] = t.stage.stage_h as f32;
                }
            }
        }
    }
    // supersampling (item 309): the taps of the affine series are computed on the CPU and passed
    // as a number; Proj series measure the Jacobian per pixel with THE SAME formula.
    // Bits 24..31 are the counter of tap ops.
    let ss = match prog.ss {
        kt_fx::geom::SsSpec::Const { nx, ny } => (nx << 8) | (ny << 16),
        kt_fx::geom::SsSpec::PerPixel => 1,
    } | ((prog.tap_ops.len() as u32) << 24);
    u.h = [prog.prims.len() as u32, kinds as u32, (kinds >> 32) as u32, ss];
    u.d = [prog.in_w, prog.in_h, prog.out_w, prog.out_h];
    u
}

#[derive(Clone, Copy)]
enum Pipe {
    Resample,
    Mirror,
    Composite,
    /// Layer blend strength (R.38 item 12): mixing of the dry and wet sides.
    LayerMix,
    // (the pointwise R.47 kinds — color_grade, duotone, posterize, vignette — are plugins)
    // R.47 barriers Kv3
    /// Embedding boundary (R.50): straight alpha from premultiplied.
    Unpremul,
    /// Premultiplication before the mip (R.53, item 418).
    Premul,
    /// One tile, copied (morpher-event-grid-cost, D2): the replicate half.
    GridTile,
    /// Mask alpha (shape-masks): the frame's alpha × the uploaded coverage.
    MaskAlpha,
}

const PIPE_COUNT: usize = 8;

/// One chain step precomputed on the CPU (common maths: kt-fx:
/// pan_frame/cam_at/noise_seed). The number of passes and the sizes are known in advance.
enum FxOp {
    /// Bilinear frame (Pan/CamMotion/Blur down+up): output tw×th.
    Resample {
        tw: u32,
        th: u32,
        frame: (f64, f64, f64, f64),
    },
    Mirror {
        h: bool,
        v: bool,
    },
    /// R.47 barriers Kv3. Sharpen/DropShadow take 3 passes (mip down/up +
    /// final), the others take one.
    /// A barrier PLUGIN's graph (plugin-api-barriers, D2): the host's nodes and its passes, `base` —
    /// its first shader in `Inner::plugin_pipes`.
    PluginBarrier {
        base: usize,
        nodes: Vec<kt_plugin_api::barrier::Node>,
    },
    /// A point-wise PLUGIN at the barrier position (plugin-api-stage1, D5): its own generated pass,
    /// `idx` — into `Inner::plugin_pipes`; `f` — `P.f0`/`P.f1` = (unused, a.yzw, b.xyzw).
    PluginTap {
        idx: usize,
        f: [f32; 8],
        /// A positioned kernel (D11.1): `b.zw` = the pass's frame size, set at execution.
        positioned: bool,
    },
    /// Merged geometry series (R.44): the program of inverse mappings is built in
    /// kt-fx (shared code with the CPU executor), one pass, a 1024 B uniform.
    /// EventGrid always takes this path (the mosaic of the former 2 passes is removed).
    GeomFused {
        prog: kt_fx::geom::GeomProgram,
    },
    /// A fused series whose grid is ONE TILE, COPIED (morpher-event-grid-cost, D2): the tile
    /// program into a cw×ch target, then the replicate pass `grid_tile.wgsl`. `prog` is the
    /// full program (its sizes and its cost), `tile` the one that computes the cell.
    GeomTile {
        prog: kt_fx::geom::GeomProgram,
        tile: kt_fx::geom::GeomProgram,
    },
    /// Snapshot of the state BEFORE a layer’s contribution (R.38 item 12): the dry side
    /// of the future mix. A marker only; it spawns no passes. `alpha` is the chain's alpha fold
    /// at this point (`None`: silent, the source's) — the dry side's alpha flag.
    Snapshot { alpha: Option<bool> },
    /// Mix the current result with the snapshot with weight `k` (0..4096).
    /// The sizes could have diverged (Pan/CamMotion resample), in which case the dry side is fitted
    /// by THE SAME bilinear as on the CPU. `alpha` is the fold here — the wet side's flag. Each
    /// side keeps its own flag, as the CPU's frames do (vhs-tape-simulation: a step that destroys
    /// alpha between the two made the end-of-chain flag wrong for the dry side, t1052).
    Mix {
        k: u32,
        alpha: Option<bool>,
    },
    /// The dry side of an effect slot's masks (shape-masks): a marker, like `Snapshot`, kept
    /// apart so a morpher snapshot and a mask snapshot never share the slot.
    MaskSnapshot,
    /// An effect slot's masks: the current result mixed with the mask snapshot per pixel by
    /// the coverage (the CPU rasterizer's bytes as a texture, `cov`).
    MaskMix {
        cov: Arc<kt_media::VideoFrame>,
        key: u64,
        /// Whether each side carries alpha (without it the byte reads as 255, as on the CPU).
        dry_alpha: bool,
        wet_alpha: bool,
    },
    /// The picture's or the track's masks: the alpha × the coverage.
    MaskAlpha {
        cov: Arc<kt_media::VideoFrame>,
        key: u64,
        has_alpha: bool,
    },
}

/// The coverage as an uploadable frame (the byte in `.r`) and its upload key: a hash of the
/// bytes with the top bit set, so it never meets a frame serial.
fn coverage_frame(c: &kt_fx::mask::Coverage) -> (Arc<kt_media::VideoFrame>, u64) {
    let mut rgba = vec![0u8; c.cov.len() * 4];
    for (px, &m) in rgba.chunks_exact_mut(4).zip(&c.cov) {
        px[0] = m;
        px[3] = 255;
    }
    let key = xxhash_rust::xxh3::xxh3_64(&c.cov) | (1u64 << 63);
    let f = kt_media::VideoFrame {
        pts: Tick::ZERO,
        width: c.w,
        height: c.h,
        rgba: rgba.into_boxed_slice(),
        has_alpha: false,
    };
    (Arc::new(f), key)
}

impl FxOp {
    fn passes(&self) -> u32 {
        match self {
            // every node is a pass but `Save` (the arena prediction stays exact)
            FxOp::PluginBarrier { nodes, .. } => {
                nodes.iter().filter(|n| !matches!(n, kt_plugin_api::barrier::Node::Save(_))).count() as u32
            }
            // a snapshot spawns no passes; when the sizes have diverged, mixing is completed
            // by resampling the dry side
            FxOp::Snapshot { .. } | FxOp::MaskSnapshot => 0,
            FxOp::Mix { .. } | FxOp::MaskMix { .. } => 2,
            FxOp::GeomTile { .. } => 2,
            _ => 1,
        }
    }

    /// Uniform arena slots (SLOT = 256 B): an ordinary pass takes one slot,
    /// a merged geometry pass takes GEOM_SLOT_UNITS. The sum MUST predict EXACTLY
    /// the consumption of the arena (debug_assert in pass/pass_geom).
    fn slot_units(&self) -> u32 {
        match self {
            FxOp::GeomFused { .. } => GEOM_SLOT_UNITS,
            // the tile pass (a geometry uniform) and the replicate pass (one slot)
            FxOp::GeomTile { .. } => GEOM_SLOT_UNITS + 1,
            _ => self.passes(),
        }
    }
}

/// Components of the ReadbackCache key from the resolved camera (R.29).
fn put_cam3d(h: &mut Xxh3, c: &Option<Cam3d>) {
    match c {
        None => h.update(&[0u8]),
        Some(c) => {
            h.update(&[1u8, c.cam.ortho as u8]);
            for v in [
                c.cam.pos_x,
                c.cam.pos_y,
                c.cam.pos_z,
                c.cam.rot_x,
                c.cam.rot_y,
                c.cam.rot_z,
                c.cam.fov_deg,
                c.world_scale,
            ] {
                h.update(&v.to_bits().to_le_bytes());
            }
            if let Some((x, y, z)) = c.cam.target {
                for v in [x, y, z] {
                    h.update(&v.to_bits().to_le_bytes());
                }
            }
        }
    }
}

/// Result of chain_ops: ops, final sizes, alpha (value/known),
/// the maximum of intermediate sizes (max_dim guard, item 314).
type ChainOps = (Vec<FxOp>, (u32, u32), bool, bool, (u32, u32));

/// The chain of one layer: ops plus the predicted output sizes/alpha.
/// A mirror of the transformations of apply_video_chain_pooled (kt-fx/src/video.rs).
#[allow(clippy::too_many_arguments)]
fn chain_ops(
    fx: &[FxInstance],
    src_w: u32,
    src_h: u32,
    src_scale: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    t: Tick,
    cam3d: Option<Cam3d>,
    // Context of the leading “Geometry” node (R.62): the same as on the CPU.
    lead: Option<kt_fx::conform::LeadCtx>,
    // The chain INPUT alpha (R.53, item 418). The `has_alpha_known` fold
    // says only whether some step has spoken; until someone has spoken, the frame’s alpha
    // is the source’s alpha, and the premultiplication before the mip must be decided by it,
    // otherwise the CPU (which has
    // `Frame.has_alpha`) and the GPU would diverge on titles.
    src_has_alpha: bool,
    // mask application points (shape-masks), at the chain positions of the CPU path
    masks: &[kt_fx::mask::MaskOp],
) -> ChainOps {
    let (mut w, mut h) = (src_w, src_h);
    // the maximum of INTERMEDIATE sizes (K×-segments, item 314): resolve_chain
    // checks it against the device max_dim, otherwise create_texture would fail
    // validation and poison the device instead of a clean TooLarge
    let (mut mw, mut mh) = (src_w, src_h);
    let mut has_alpha_known = false; // whether an effect set the alpha explicitly
    let mut has_alpha = false;
    let mut ops = Vec::new();
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    // the boundaries of layer contributions (R.38 item 12): a snapshot of the dry
    // side of the future mix is taken at them
    let froms: Vec<usize> = fx
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            FxInstance::LayerMix { from, .. } => Some((*from as usize).min(i)),
            _ => None,
        })
        .collect();
    // The chain plan is built by kt-fx (R.45, item 311): the SHARED code of both backends
    // (invariant 7); here the ops are only translated into FxOp.
    // mask points cut the chain too: the same slices as `apply_chain_with_mixes` on the CPU
    let mut stops = froms.clone();
    stops.extend(kt_fx::mask::mask_stops(masks));
    stops.sort_unstable();
    stops.dedup();
    let plan = kt_fx::plan::plan_chain_in(
        fx, src_w, src_h, src_scale, out_w, out_h, u, cam3d, t.0, &stops, lead,
    );
    let s = src_scale.max(1);
    let in_logical = (src_w / s, src_h / s);
    // the chain position the next plan op starts at (mask points are emitted at positions)
    let mut pos = 0usize;
    // the alpha at the mask snapshot (the dry side's flag of the mix)
    let mut snap_alpha = false;
    let mask_points = |at: usize, ops: &mut Vec<FxOp>, w: u32, h: u32, cur_alpha: bool, snap_alpha: &mut bool| -> bool {
        use kt_fx::mask::MaskOpKind;
        let mut produced = false;
        for op in masks.iter().filter(|o| o.at == at && matches!(o.kind, MaskOpKind::Mix { .. })) {
            let sp = kt_fx::mask::mask_space(op, fx, in_logical, (out_w, out_h), (w, h), lead);
            let (cov, key) = coverage_frame(&kt_fx::mask::rasterize_cached(&op.masks, &sp, w, h));
            ops.push(FxOp::MaskMix { cov, key, dry_alpha: *snap_alpha, wet_alpha: cur_alpha });
            produced |= *snap_alpha;
        }
        for op in masks.iter().filter(|o| o.at == at && o.kind == MaskOpKind::Alpha) {
            let sp = kt_fx::mask::mask_space(op, fx, in_logical, (out_w, out_h), (w, h), lead);
            let (cov, key) = coverage_frame(&kt_fx::mask::rasterize_cached(&op.masks, &sp, w, h));
            ops.push(FxOp::MaskAlpha { cov, key, has_alpha: cur_alpha || produced });
            produced = true;
        }
        if masks.iter().any(|o| matches!(o.kind, MaskOpKind::Mix { from } if from == at)) {
            ops.push(FxOp::MaskSnapshot);
            *snap_alpha = cur_alpha || produced;
        }
        produced
    };
    // open snapshots: (op position, frame size there) — for the strength-0 skip below
    // (op index, size, the alpha fold at the snapshot)
    let mut snaps: Vec<(usize, u32, u32, Option<bool>)> = Vec::new();
    for op in plan.ops {
        // where this op starts in the chain; the mask points standing there come first
        let span = match &op {
            kt_fx::plan::PlanOp::Run { i, j, .. } => Some((*i, *j)),
            kt_fx::plan::PlanOp::Barrier { idx, .. } | kt_fx::plan::PlanOp::Mix { idx } => {
                Some((*idx, *idx + 1))
            }
            kt_fx::plan::PlanOp::Snapshot => None,
        };
        let start = span.map(|(i, _)| i);
        if let Some(st) = start {
            while pos <= st {
                let cur = if has_alpha_known { has_alpha } else { src_has_alpha };
                if !masks.is_empty() && mask_points(pos, &mut ops, w, h, cur, &mut snap_alpha) {
                    // the alpha rule (R.53): a mask PRODUCES transparency
                    has_alpha_known = true;
                    has_alpha = true;
                }
                pos += 1;
            }
        }
        match op {
            kt_fx::plan::PlanOp::Snapshot => {
                snaps.push((ops.len(), w, h, has_alpha_known.then_some(has_alpha)));
                ops.push(FxOp::Snapshot { alpha: has_alpha_known.then_some(has_alpha) })
            }
            kt_fx::plan::PlanOp::Mix { idx } => {
                let single = snaps.len() == 1;
                let snap = snaps.pop();
                if let FxInstance::LayerMix { amount, .. } = &fx[idx] {
                    let k = (amount.clamp(0.0, 1.0) * 4096.0).round() as u32;
                    // STRENGTH EXACTLY 0 (morpher-event-grid-cost, D3): the slice's ops are
                    // dropped and the mix runs on (dry, dry) — at k = 0 the wet side contributes
                    // nothing, so the bytes are those of the full path. Only for a lone open
                    // snapshot whose frame size the slice did not change (no resample in the mix);
                    // the alpha fold keeps what the slice would have declared.
                    let dry_alpha = snap.and_then(|s| s.3);
                    if let Some((at, sw, sh, _)) = snap
                        && k == 0
                        && single
                        && (sw, sh) == (w, h)
                    {
                        ops.truncate(at + 1);
                    }
                    if k < 4096 {
                        ops.push(FxOp::Mix { k, alpha: has_alpha_known.then_some(has_alpha) });
                        // the mixed frame has alpha when either side has (the CPU's
                        // `blend_frames_cpu`): a step that destroyed alpha after the snapshot does
                        // not make the mix opaque (vhs-tape-simulation, t1052)
                        if snap.is_some() {
                            let wet = if has_alpha_known { has_alpha } else { src_has_alpha };
                            let dry = dry_alpha.unwrap_or(src_has_alpha);
                            has_alpha_known = true;
                            has_alpha = dry || wet;
                        }
                    }
                }
            }
            kt_fx::plan::PlanOp::Run { i, j, exec } => match exec {
                kt_fx::geom::GeomExec::Legacy => match fx[i..j]
                    .iter()
                    .find(|f| kt_fx::geom::is_geo_active(f, lead, out_w, out_h))
                    .expect("legacy run has a geo step")
                {
                    FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
                        let (px, py, z) = kt_fx::video::cam_at(keyframes, u, *ease, *time_scale);
                        let frame = kt_fx::video::pan_frame(w, h, out_w, out_h, *mode, px, py, z);
                        ops.push(FxOp::Resample { tw: mow, th: moh, frame });
                        w = mow;
                        h = moh;
                    }
                    f @ FxInstance::Plugin(_) => {
                        if let Some(kt_plugin_api::Lone::Mirror { h, v }) = kt_fx::geom::plugin_lone(f)
                            && (h || v)
                        {
                            ops.push(FxOp::Mirror { h, v });
                        }
                    }
                    _ => debug_assert!(false, "legacy geo must be CamMotion/Mirror"),
                },
                kt_fx::geom::GeomExec::Fused(prog) => {
                    w = prog.out_w;
                    h = prog.out_h;
                    mw = mw.max(w);
                    mh = mh.max(h);
                    if prog.has_alpha_known {
                        has_alpha_known = true;
                        has_alpha = prog.has_alpha;
                    }
                    match prog.tile_program() {
                        Some(tile) => ops.push(FxOp::GeomTile { prog, tile }),
                        None => ops.push(FxOp::GeomFused { prog }),
                    }
                }
            },
            kt_fx::plan::PlanOp::Barrier { idx, res_scale } => match &fx[idx] {
                f @ FxInstance::Plugin(slot) if kt_fx::video::plugin_barrier(f).is_some() => {
                    // a barrier plugin (plugin-api-barriers): its graph against the frame here; an empty
                    // graph is a passthrough (drop_shadow on an opaque frame)
                    let (b, values) = kt_fx::video::plugin_barrier(f).expect("checked");
                    let g = kt_plugin_api::barrier::GpuCtx {
                        w,
                        h,
                        res_scale,
                        alpha: has_alpha_known.then_some(has_alpha),
                        src_has_alpha,
                        burst_index: t.0,
                    };
                    let mut nodes = Vec::new();
                    b.gpu_values(values, &g, &mut nodes);
                    debug_assert_eq!(
                        kt_plugin_api::barrier::check_graph(&nodes, b.shaders().len(), &g, kt_fx::plan::MAX_INTER_DIM),
                        Ok(()),
                        "{}: a barrier graph the registry would refuse",
                        slot.manifest().id
                    );
                    for n in &nodes {
                        if let kt_plugin_api::barrier::Node::Resample { w: rw, h: rh }
                        | kt_plugin_api::barrier::Node::PassAt { w: rw, h: rh, .. } = n
                        {
                            mw = mw.max(*rw);
                            mh = mh.max(*rh);
                        }
                    }
                    if !nodes.is_empty() {
                        ops.push(FxOp::PluginBarrier { base: barrier_pipe_base(&slot.manifest().id), nodes });
                    }
                    // the alpha rule (R.53, item 417): declared by the step, not matched by kind
                    match slot.manifest().decl.alpha {
                        kt_plugin_api::barrier::AlphaRel::Preserves => {}
                        kt_plugin_api::barrier::AlphaRel::Produces => {
                            has_alpha_known = true;
                            has_alpha = true;
                        }
                        kt_plugin_api::barrier::AlphaRel::Destroys => {
                            has_alpha_known = true;
                            has_alpha = false;
                        }
                    }
                }
                f @ FxInstance::Plugin(_) if kt_fx::geom::PluginTap::of(f).is_some() => {
                    let t = kt_fx::geom::PluginTap::of(f).expect("checked");
                    let g = t.gpu();
                    let idx = plugin_taps().iter().position(|(id, _)| *id == t.id).expect("assembled at engine creation");
                    ops.push(FxOp::PluginTap {
                        idx,
                        f: [0.0, g[0], g[1], g[2], g[3], g[4], g[5], g[6]],
                        positioned: t.positioned,
                    });
                }
                _ => debug_assert!(false, "barrier op on non-barrier fx"),
            },
        }
        // the op consumed the chain up to its end (a run never spans a stop: no mask point
        // lies inside it)
        if let Some((_, end)) = span {
            pos = pos.max(end);
        }
    }
    // the points after the last op, the chain's end included (a track's masks)
    if !masks.is_empty() {
        while pos <= fx.len() {
            let cur = if has_alpha_known { has_alpha } else { src_has_alpha };
            if mask_points(pos, &mut ops, w, h, cur, &mut snap_alpha) {
                has_alpha_known = true;
                has_alpha = true;
            }
            pos += 1;
        }
    }
    (ops, (w, h), has_alpha, has_alpha_known, (mw, mh))
}

/// Where the layer’s chain takes its input from.
enum ChainSrc {
    /// The loaded source frame.
    Tex(wgpu::TextureView),
    /// The SUBTREE of an embedding (R.50): its own jobs, its own size. It has no texture
    /// yet at parse time: it appears at encoding, so parsing and encoding
    /// had to be split: the uniform arena is allocated BEFORE the encoder,
    /// and its size depends on the whole tree.
    Nest(Vec<Job>),
}

struct ChainSpec {
    ops: Vec<FxOp>,
    src: ChainSrc,
    src_w: u32,
    src_h: u32,
    has_alpha: bool, // after the chain
    /// The chain INPUT's alpha (the source frame's, or a nested reel's composite): a silent fold
    /// at a `Snapshot`/`Mix` means this.
    src_has_alpha: bool,
}

impl ChainSpec {
    fn slot_units(&self) -> u32 {
        let own: u32 = self.ops.iter().map(|o| o.slot_units()).sum();
        match &self.src {
            ChainSrc::Tex(_) => own,
            // + 1 per pass removing premultiplication at the boundary
            ChainSrc::Nest(jobs) => own + 1 + jobs.iter().map(|j| j.slot_units()).sum::<u32>(),
        }
    }
}

enum Job {
    Single {
        chain: ChainSpec,
        alpha_k: u32,
        scale: bool,
        fade: Option<SoloFade>,
        /// The blend mode of a layer downward (R.62). `Normal` is the old path
        /// byte for byte; the rest is computed by the shader’s `blend_over`, a mirror of the
        /// CPU kernel `kt_engine::video::blend_ch`.
        blend_mode: kt_model::BlendMode,
    },
    Dissolve {
        a: ChainSpec,
        b: ChainSpec,
        k: u32,
        alpha_k: u32,
        mix: TransMix,
    },
}

impl ChainSpec {
    /// What the chain runs (morpher-event-grid-cost, D1), nested reels included.
    fn cost(&self) -> kt_fx::video::FrameCost {
        let mut c = kt_fx::video::FrameCost::default();
        for op in &self.ops {
            match op {
                FxOp::GeomFused { prog } => c.add(kt_fx::video::FrameCost::geom(prog)),
                FxOp::GeomTile { prog, .. } => {
                    c.add(kt_fx::video::FrameCost::geom(prog));
                    c.passes += 1;
                }
                FxOp::Snapshot { .. } | FxOp::MaskSnapshot => c.snapshots += 1,
                FxOp::Mix { .. } | FxOp::MaskMix { .. } => {
                    c.mixes += 1;
                    c.passes += op.passes();
                }
                _ => c.passes += op.passes(),
            }
        }
        if let ChainSrc::Nest(jobs) = &self.src {
            for j in jobs {
                c.add(j.cost());
            }
        }
        c
    }
}

impl Job {
    fn cost(&self) -> kt_fx::video::FrameCost {
        match self {
            Job::Single { chain, .. } => chain.cost(),
            Job::Dissolve { a, b, .. } => {
                let mut c = a.cost();
                c.add(b.cost());
                c
            }
        }
    }

    /// Uniform arena slots per job: the composite pass plus the layer chains.
    fn slot_units(&self) -> u32 {
        1 + match self {
            Job::Single { chain, .. } => chain.slot_units(),
            Job::Dissolve { a, b, .. } => a.slot_units() + b.slot_units(),
        }
    }
}

struct Inner {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    layout: wgpu::BindGroupLayout,
    pipes: [wgpu::RenderPipeline; PIPE_COUNT],
    /// The own passes of the point-wise plugins, in `plugin_taps()` order (D5).
    plugin_pipes: Vec<wgpu::RenderPipeline>,
    /// The merged geometry pass (R.44): its own 1024 B uniform → its own layout/pipe.
    geom_layout: wgpu::BindGroupLayout,
    geom_pipe: wgpu::RenderPipeline,
    arena: wgpu::Buffer,
    arena_slots: u32,
    slot: u32,
    pool: TexturePool,
    upload: UploadCache,
    readback: ReadbackCache,
    /// What the last `execute` ran (morpher-event-grid-cost, D1).
    last_cost: kt_fx::video::FrameCost,
    staging: Option<(wgpu::Buffer, u32, u32, u32)>, // (buf, padded_bpr, w, h)
    dummy: Tex,
    max_dim: u32,
}

pub struct GpuFxEngine {
    info: GpuBackendInfo,
    inner: Mutex<Inner>,
}

const SHADERS: [(&str, &str); PIPE_COUNT] = [
    ("resample", include_str!("shaders/resample.wgsl")),
    ("mirror", include_str!("shaders/mirror.wgsl")),
    ("composite", include_str!("shaders/composite.wgsl")),
    ("layer_mix", include_str!("shaders/layer_mix.wgsl")),
    ("unpremul", include_str!("shaders/unpremul.wgsl")),
    ("premul", include_str!("shaders/premul.wgsl")),
    ("grid_tile", include_str!("shaders/grid_tile.wgsl")),
    ("mask", include_str!("shaders/mask.wgsl")),
];

const COMMON: &str = include_str!("shaders/common.wgsl");

/// Where a pass's pipeline comes from.
#[derive(Clone, Copy)]
enum Which {
    Fixed(Pipe),
    Plugin(usize),
}

/// The transition plugins of the installed registry with their `trans_<id>` WGSL, in registration
/// order (D12): a kind's mode is `MODE_PLUGIN + its index here`, as on the CPU.
pub fn plugin_transitions() -> Vec<(&'static str, &'static str)> {
    let Some(reg) = kt_plugin_api::installed() else { return Vec::new() };
    reg.transitions()
        .into_iter()
        .filter_map(|p| {
            let m = p.manifest();
            match m.decl.gpu {
                kt_plugin_api::Gpu::Wgsl(w) => {
                    let id: &'static str = &m.id;
                    Some((id, w))
                }
                _ => None,
            }
        })
        .collect()
}

/// composite.wgsl with one arm per transition plugin and their functions appended (D12): a pure
/// function of the registry, like the fused pass's text.
pub fn assembled_composite() -> String {
    let marker = "    // @PLUGIN_TRANS_ARMS";
    let base = include_str!("shaders/composite.wgsl");
    assert!(base.contains(marker), "composite.wgsl lost its transition arm marker");
    let mut arms = String::new();
    let mut fns = String::new();
    for (i, (id, wgsl)) in plugin_transitions().iter().enumerate() {
        let mode = kt_engine::transitions::MODE_PLUGIN + i as u32;
        arms.push_str(&format!("    if (mode == {mode}u) {{ return trans_{id}(uv, k); }}\n"));
        fns.push_str(wgsl);
        fns.push('\n');
    }
    base.replacen(marker, &format!("{arms}{marker}"), 1) + "\n" + &fns
}

/// The point-wise plugins of the installed registry that offer WGSL, in registration order:
/// (id, the `tap_<id>` function). The ONE list the assembler, the uniform packing and `chain_ops`
/// read, so a dispatch number and an own-pass index can never disagree (plugin-api-stage1, D5).
pub fn plugin_taps() -> Vec<(&'static str, &'static str)> {
    let Some(reg) = kt_plugin_api::installed() else { return Vec::new() };
    reg.plugins()
        .filter_map(|p| {
            let m = p.manifest();
            match (m.media, m.decl.video_class, m.decl.gpu) {
                (kt_plugin_api::Media::VideoFx, Some(kt_plugin_api::VideoClass::PointWise), kt_plugin_api::Gpu::Wgsl(w)) => {
                    let id: &'static str = &m.id;
                    Some((id, w))
                }
                _ => None,
            }
        })
        .collect()
}

/// The barrier plugins of the installed registry with their fragment passes, in registration order
/// (plugin-api-barriers, D2). Their pipelines follow the point-wise plugins' own passes in
/// `Inner::plugin_pipes`; `barrier_pipe_base` finds a plugin's first one. A function of the registry
/// only, never hashed into a key (the chain's key carries the plugin id).
pub fn plugin_barriers() -> Vec<(&'static str, &'static [&'static str])> {
    let Some(reg) = kt_plugin_api::installed() else { return Vec::new() };
    reg.plugins()
        .filter_map(|p| {
            let m = p.manifest();
            // a barrier, or a point-wise plugin with a barrier kernel of its own (noise, greenscreen)
            match (m.media, m.decl.gpu, p.barrier()) {
                (kt_plugin_api::Media::VideoFx, g, Some(b)) if g != kt_plugin_api::Gpu::None => {
                    let id: &'static str = &m.id;
                    Some((id, b.shaders()))
                }
                _ => None,
            }
        })
        .collect()
}

/// The index of a barrier plugin's first pass in `Inner::plugin_pipes`.
fn barrier_pipe_base(id: &str) -> usize {
    let mut base = plugin_taps().len();
    for (p, shaders) in plugin_barriers() {
        if p == id {
            return base;
        }
        base += shaders.len();
    }
    panic!("a barrier of an unassembled plugin: {id}")
}

/// The geometry plugins of the installed registry that offer a warp map, in registration order:
/// (id, the `warp_<id>` function). The ONE list the assembler and the uniform packing read (D11.2).
pub fn plugin_warps() -> Vec<(&'static str, &'static str)> {
    let Some(reg) = kt_plugin_api::installed() else { return Vec::new() };
    reg.plugins()
        .filter_map(|p| {
            let m = p.manifest();
            match (m.decl.video_class, m.decl.gpu) {
                (Some(kt_plugin_api::VideoClass::Geometry), kt_plugin_api::Gpu::Warp(w)) => {
                    let id: &'static str = &m.id;
                    Some((id, w))
                }
                _ => None,
            }
        })
        .collect()
}

/// The warp dispatch number of a geometry plugin (the `mode` in the primitive's first slot): its
/// index in `plugin_warps`. A function of the registry only, never hashed into a key.
fn plugin_warp_mode(id: &str) -> u32 {
    plugin_warps().iter().position(|(p, _)| *p == id).expect("a warp of an unassembled plugin") as u32
}

/// The tap dispatch number of a point-wise plugin in the fused pass: after the built-in kinds
/// 0..5 (4 was Posterize). It is a function of the registry only, and NEVER hashed into a key —
/// the keys carry the plugin id (the chain's Debug form).
fn plugin_tap_kind(id: &str) -> u32 {
    let i = plugin_taps().iter().position(|(p, _)| *p == id).expect("a tap of an unassembled plugin");
    let kind = 6 + i as u32;
    assert!(kind < 16, "the tap kind field is 4 bits: at most 10 point-wise plugins");
    kind
}

/// The shader texts assembled from the registry (D5): the fused pass with one dispatch arm per
/// point-wise plugin and their functions appended, and one own pass per plugin. A pure function of
/// the registry — two engines of one process get the same text (t974, invariant 11).
pub fn assembled_shaders() -> (String, Vec<String>) {
    let taps = plugin_taps();
    let mut arms = String::new();
    let mut fns = String::new();
    let mut own = Vec::new();
    for (i, (id, wgsl)) in taps.iter().enumerate() {
        arms.push_str(&format!("        }} else if (tap_kind == {}u) {{ v = tap_{id}(v, a, b, rec[t]);\n", 6 + i));
        fns.push_str(wgsl);
        fns.push('\n');
        own.push(format!(
            "{wgsl}\n@fragment\nfn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {{\n    \
             let t = textureLoad(T0, vec2<i32>(pos.xy), 0);\n    \
             let v = tap_{id}(vec4<f32>(f32(t.x), f32(t.y), f32(t.z), f32(t.w)), P.f0, P.f1, floor(pos.xy));\n    \
             return vec4<u32>(clamp(round(v), vec4<f32>(0.0), vec4<f32>(255.0)));\n}}\n"
        ));
    }
    // the warp maps (D11.2): one arm per plugin in BOTH primitive loops, one function each
    let mut warp_arms = String::new();
    for (i, (id, wgsl)) in plugin_warps().iter().enumerate() {
        warp_arms.push_str(&format!(
            "                if (mode == {i}u) {{ let q = warp_{id}(vec2<f32>(x, y), a, b); x = q.x; y = q.y; }}\n"
        ));
        fns.push_str(wgsl);
        fns.push('\n');
    }
    let marker = "        // @PLUGIN_TAP_ARMS";
    let warp_marker = "                // @PLUGIN_WARP_ARMS";
    let base = include_str!("shaders/geom_fused.wgsl");
    assert!(base.contains(marker), "geom_fused.wgsl lost its plugin arm marker");
    assert_eq!(base.matches(warp_marker).count(), 2, "geom_fused.wgsl lost a warp arm marker (both loops)");
    let fused = base
        .replacen(marker, &format!("{arms}{marker}"), 1)
        .replace(warp_marker, &format!("{warp_arms}{warp_marker}"))
        + "\n"
        + &fns;
    (fused, own)
}

impl GpuFxEngine {
    /// Builds the pipelines; a shader/device compile error → Err
    /// (the caller falls back to the CPU with a notice).
    pub fn new(ctx: GpuContext) -> Result<Arc<GpuFxEngine>, GpuInitError> {
        let device = ctx.device;
        let queue = ctx.queue;
        let ai = &ctx.adapter_info;
        let name_l = ai.name.to_lowercase();
        let info = GpuBackendInfo {
            name: ai.name.clone(),
            backend: format!("{:?}", ai.backend),
            software: ai.device_type == wgpu::DeviceType::Cpu
                || name_l.contains("llvmpipe")
                || name_l.contains("softpipe")
                || name_l.contains("swiftshader")
                // Windows' software rasterizer (WARP)
                || name_l.contains("microsoft basic render"),
        };

        // WARP ("Microsoft Basic Render Driver", Windows' software D3D12 rasterizer) is
        // refused: on the GPU-less Windows CI runner the engine's passes ran for minutes and
        // then died inside it (access violations, heap corruption — windows-build). The CPU
        // path is the full fallback and the byte reference (invariant 7), so a machine with
        // only WARP renders effects on the CPU instead of risking a crash.
        // KT_GPU_ALLOW_WARP=1 lets it through for experiments.
        if name_l.contains("microsoft basic render")
            && std::env::var("KT_GPU_ALLOW_WARP").as_deref() != Ok("1")
        {
            return Err(GpuInitError::Unsupported(format!(
                "{} (software D3D12): effects render on the CPU",
                ai.name
            )));
        }
        // Two scopes: a shader the backend's compiler rejects (FXC on D3D12: "not natively
        // addressable", windows-build) is an INTERNAL error, not a validation one. With only
        // the validation scope it escaped to the default handler, which panics — a crash on
        // Windows instead of the CPU fallback (invariant 7). Popped in reverse order.
        let internal_scope = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let err_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kt-gpu layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // The VERTEX stage also reads the uniform (vs3d Pan3d:
                    // MVP/anchor): FRAGMENT-only made create_render_pipeline fail
                    // on real hardware and dropped the WHOLE engine to the CPU (R.31 item 1)
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(128),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
            ],
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kt-gpu pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let (fused_src, plugin_srcs) = assembled_shaders();
        let composite = assembled_composite();
        let fixed: Vec<(String, String)> = SHADERS
            .iter()
            .map(|(n, b)| (n.to_string(), if *n == "composite" { composite.clone() } else { b.to_string() }))
            .collect();
        let mut plugins: Vec<(String, String)> =
            plugin_taps().iter().zip(&plugin_srcs).map(|((id, _), body)| (format!("plugin {id}"), body.clone())).collect();
        // the barrier plugins' passes follow, in `plugin_barriers` order (`barrier_pipe_base`)
        for (id, shaders) in plugin_barriers() {
            for (i, body) in shaders.iter().enumerate() {
                plugins.push((format!("barrier {id} {i}"), body.to_string()));
            }
        }
        let mut pipes: Vec<wgpu::RenderPipeline> = fixed
            .iter()
            .chain(plugins.iter())
            .map(|(name, body)| {
                let name = name.as_str();
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(name),
                    source: wgpu::ShaderSource::Wgsl(format!("{COMMON}\n{body}").into()),
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(name),
                    layout: Some(&pipe_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .collect();
        // The merged geometry pass (R.44): the GEOM_UNIFORM_BYTES uniform does not fit into
        // the shared layout (128 B) — its own layout and pipe from the same arena buffer.
        let geom_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kt-gpu geom layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(GEOM_UNIFORM_BYTES as u64),
                    },
                    count: None,
                },
                texture_entry(1),
            ],
        });
        let geom_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kt-gpu geom pipeline layout"),
            bind_group_layouts: &[Some(&geom_layout)],
            immediate_size: 0,
        });
        let geom_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("geom_fused"),
            source: wgpu::ShaderSource::Wgsl(fused_src.into()),
        });
        let geom_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("geom_fused"),
            layout: Some(&geom_pipe_layout),
            vertex: wgpu::VertexState {
                module: &geom_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &geom_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let validation = pollster::block_on(err_scope.pop());
        let internal = pollster::block_on(internal_scope.pop());
        if let Some(e) = validation.or(internal) {
            return Err(GpuInitError::Pipeline(e.to_string()));
        }
        let plugin_pipes = pipes.split_off(PIPE_COUNT);
        let pipes: [wgpu::RenderPipeline; PIPE_COUNT] =
            pipes.try_into().map_err(|_| GpuInitError::Pipeline("pipe count".into()))?;

        let arena_slots = 256u32;
        let arena = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kt-gpu uniforms"),
            size: SLOT * arena_slots as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy = create_tex(&device, 1, 1, "kt-gpu dummy");
        let max_dim = device.limits().max_texture_dimension_2d;
        Ok(Arc::new(GpuFxEngine {
            info,
            inner: Mutex::new(Inner {
                device,
                queue,
                layout,
                pipes,
                plugin_pipes,
                geom_layout,
                geom_pipe,
                arena,
                arena_slots,
                slot: 0,
                pool: TexturePool::new(TRANSIENT_BUDGET),
                upload: UploadCache::new(UPLOAD_BUDGET),
                readback: ReadbackCache::new(READBACK_BUDGET),
                last_cost: Default::default(),
                staging: None,
                dummy,
                max_dim,
            }),
        }))
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Uint,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl VideoGpuExec for GpuFxEngine {
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.inner.lock().unwrap().execute(plan, out)
    }

    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }

    fn last_cost(&self) -> kt_fx::video::FrameCost {
        self.inner.lock().unwrap().last_cost
    }
}

impl Inner {
    fn execute(&mut self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        let (ow, oh) = (plan.out_w, plan.out_h);
        if ow > self.max_dim || oh > self.max_dim {
            return Err(GpuExecError::TooLarge { w: ow, h: oh });
        }
        let key = plan_key(plan);
        self.last_cost = Default::default();
        if let Some((data, complete)) = self.readback.get(key) {
            out.clear();
            out.extend_from_slice(data);
            return Ok(complete);
        }

        // --- resolve: presence/size by the rules of the CPU executor ---
        let (jobs, complete) = self.resolve_plan(plan)?;
        for j in &jobs {
            self.last_cost.add(j.cost());
        }

        let n = (ow as usize) * (oh as usize) * 4;
        if jobs.is_empty() {
            out.clear();
            out.resize(n, 0);
            return Ok(complete);
        }

        // --- uniform arena for all passes (in slot units: a geometry pass = 4) ---
        let total: u32 = jobs.iter().map(|j| j.slot_units()).sum();
        if total > self.arena_slots {
            let slots = total.next_power_of_two();
            self.arena = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu uniforms"),
                size: SLOT * slots as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.arena_slots = slots;
        }
        self.slot = 0;

        // --- encode ---
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kt-gpu") });
        let result = self.encode_jobs(&mut enc, &jobs, ow, oh);

        // --- readback ---
        let padded_bpr = (ow * 4).next_multiple_of(256);
        let need = (padded_bpr as u64) * (oh as u64);
        let recreate = match &self.staging {
            Some((_, bpr, w, h)) => *bpr != padded_bpr || *w != ow || *h != oh,
            None => true,
        };
        if recreate {
            self.staging = Some((
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("kt-gpu staging"),
                    size: need,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                padded_bpr,
                ow,
                oh,
            ));
        }
        let (staging, _, _, _) = self.staging.as_ref().unwrap();
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &result.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bpr),
                    rows_per_image: Some(oh),
                },
            },
            wgpu::Extent3d { width: ow, height: oh, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| GpuExecError::DeviceLost)?;
        match rx.recv() {
            Ok(Ok(())) => {}
            _ => return Err(GpuExecError::DeviceLost),
        }
        out.clear();
        out.reserve(n);
        {
            let data = slice.get_mapped_range().map_err(|_| GpuExecError::DeviceLost)?;
            for row in 0..oh as usize {
                let s = row * padded_bpr as usize;
                out.extend_from_slice(&data[s..s + ow as usize * 4]);
            }
        }
        staging.unmap();

        self.pool.release(result);
        self.readback.insert(key, out.clone(), complete);
        Ok(complete)
    }

    /// Encodes a set of jobs into an accumulator texture. ONE code at the root and in
    /// a subtree of an embedding (R.50): otherwise the rule “the first layer writes over the
    /// transparent one” would have to be kept in sync in two places.
    fn encode_jobs(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        jobs: &[Job],
        ow: u32,
        oh: u32,
    ) -> Tex {
        let acc_a = self.pool.acquire(&self.device, ow, oh);
        let acc_b = self.pool.acquire(&self.device, ow, oh);
        let acc = [acc_a, acc_b];
        let mut cur = 0usize; // where the LAST composite pass wrote
        if jobs.is_empty() {
            // The pool gives back DIRTY textures (t183): an empty plan MUST
            // clear its own, otherwise the screen shows remnants of a foreign frame.
            self.clear_tex(enc, &acc[cur].view);
        }
        for (ji, job) in jobs.iter().enumerate() {
            let first = (ji == 0) as u32;
            let target = 1 - cur;
            match job {
                Job::Single { chain, alpha_k, scale, fade, blend_mode } => {
                    let (view, owned) = self.encode_chain(enc, chain);
                    // BLEND MODE (R.62): its own mode of the pass: the level
                    // goes into the kernel, not into the alpha (two axes, both after
                    // alpha). Normal takes the former branches.
                    let blend_id =
                        kt_model::BlendMode::ALL.iter().position(|m| m == blend_mode).unwrap_or(0)
                            as u32;
                    let mode = if blend_id != 0 {
                        4
                    } else if fade.is_some() {
                        3
                    } else if *scale {
                        1
                    } else {
                        0
                    };
                    // a solo fade with a view: the frame stands on its own side of the
                    // pair, the second is marked empty (clear bit), and the kernel
                    // does not read it at all (a mirror of Img::Clear)
                    let has = chain.has_alpha as u32;
                    let (u0z, u0w, clear, swap) = match fade {
                        Some(f) if f.incoming => (0, has, 1u32, true),
                        Some(_) => (has, 0, 2u32, false),
                        None => (has, 0, 0, false),
                    };
                    let u = UniformData {
                        u: [
                            mode,
                            *alpha_k,
                            u0z,
                            u0w,
                            first,
                            // with mode == 4 (blend) there is no fade,
                            // and slot `k` takes the MODE NUMBER: it has
                            // no cell in the uniform, and these two things
                            // are mutually exclusive by construction
                            if mode == 4 { blend_id } else { fade.map(|f| f.k).unwrap_or(0) },
                            fade.map(|f| f.mix.mode).unwrap_or(0),
                            clear,
                        ],
                        f: fade.map(|f| f.mix.p).unwrap_or_default(),
                        // volumetric kinds (R.51, item 401): panel homographies,
                        // computed in f64 by the shared trans_ext: both backends
                        // read the same f32 bits
                        m: fade
                            .map(|f| kt_engine::transitions::trans_ext(&f.mix, f.k))
                            .unwrap_or_default(),
                    };
                    let acc_in = acc[cur].view.clone();
                    let dummy = self.dummy.view.clone();
                    let (t1, t2) = if swap { (&dummy, &view) } else { (&view, &dummy) };
                    self.pass(enc, Pipe::Composite, &acc[target].view, &acc_in, t1, t2, u);
                    for t in owned {
                        self.pool.release(t);
                    }
                }
                Job::Dissolve { a, b, k, alpha_k, mix } => {
                    let (va, oa) = self.encode_chain(enc, a);
                    let (vb, ob) = self.encode_chain(enc, b);
                    let u = UniformData {
                        u: [
                            2,
                            *alpha_k,
                            a.has_alpha as u32,
                            b.has_alpha as u32,
                            first,
                            *k,
                            mix.mode,
                            0,
                        ],
                        f: mix.p,
                        m: kt_engine::transitions::trans_ext(mix, *k),
                    };
                    let acc_in = acc[cur].view.clone();
                    self.pass(enc, Pipe::Composite, &acc[target].view, &acc_in, &va, &vb, u);
                    for t in oa.into_iter().chain(ob) {
                        self.pool.release(t);
                    }
                }
            }
            cur = target;
        }

        let [a0, a1] = acc;
        if cur == 0 {
            self.pool.release(a1);
            a0
        } else {
            self.pool.release(a0);
            a1
        }
    }

    /// A transparent-black texture (the pool gives back dirty ones).
    fn clear_tex(&mut self, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kt-gpu clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    /// Parses the PLAN into jobs (without the encoder). The recursion lives here: the
    /// uniform arena is allocated BEFORE encoding, and its size depends on the WHOLE
    /// tree (R.50, B3).
    fn resolve_plan(&mut self, plan: &FramePlan<'_>) -> Result<(Vec<Job>, bool), GpuExecError> {
        let (ow, oh) = (plan.out_w, plan.out_h);
        if ow > self.max_dim || oh > self.max_dim {
            return Err(GpuExecError::TooLarge { w: ow, h: oh });
        }
        let mut complete = true;
        let mut jobs: Vec<Job> = Vec::new();
        for layer in &plan.layers {
            match layer {
                PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                    match self.resolve_chain(src, ow, oh, plan.t)? {
                        None => complete = false,
                        Some((chain, fits, sub_ok)) => {
                            complete &= sub_ok;
                            if fits {
                                jobs.push(Job::Single {
                                    chain,
                                    alpha_k: *alpha_k,
                                    scale: *scale_alpha,
                                    fade: *fade,
                                    blend_mode: src.blend_mode,
                                });
                            } else {
                                // R.58, item 459: a foreign size no longer
                                // passes silently. For a reel it is our
                                // arithmetic (`NestSrc::fit_to`); for a file it is
                                // the decoder’s message; in both cases the frame
                                // is not full.
                                debug_assert!(
                                    src.input.nest().is_none(),
                                    "nest chain ends at a foreign size, plan is {ow}x{oh}"
                                );
                                complete = false;
                            }
                        }
                    }
                }
                PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                    let ca = self.resolve_chain(a, ow, oh, plan.t)?;
                    let cb = self.resolve_chain(b, ow, oh, plan.t)?;
                    complete &= ca.as_ref().map(|c| c.2).unwrap_or(true);
                    complete &= cb.as_ref().map(|c| c.2).unwrap_or(true);
                    match (ca, cb) {
                        (Some((a, true, _)), Some((b, true, _))) => {
                            jobs.push(Job::Dissolve { a, b, k: *k, alpha_k: *alpha_k, mix: *mix });
                        }
                        (Some((c, true, _)), None) | (None, Some((c, true, _))) => {
                            jobs.push(Job::Single {
                                chain: c,
                                alpha_k: *alpha_k,
                                scale: *alpha_k < 4096,
                                fade: None,
                                // one half of a transition pair is an ordinary
                                // blend: the mode lives on the layer-track
                                blend_mode: kt_model::BlendMode::Normal,
                            });
                            complete = false;
                        }
                        _ => complete = false,
                    }
                }
            }
        }
        Ok((jobs, complete))
    }

    /// None — no frame starvation; Some((chain, fits, complete)) — the chain
    /// is ready, `fits` = the final size matched Quality, `complete` —
    /// the completeness of the SUBTREE (an ordinary layer is always true).
    fn resolve_chain(
        &mut self,
        src: &LayerSrc<'_>,
        ow: u32,
        oh: u32,
        t: Tick,
    ) -> Result<Option<(ChainSpec, bool, bool)>, GpuExecError> {
        // EMBEDDING: the chain input is not a frame from the cache but a sub-composite. Its
        // texture does not exist now, and that is normal: `ChainSrc::Nest` carries
        // the jobs, and the texture appears at encoding.
        if let Some(n) = src.input.nest() {
            let (jobs, sub_ok) = self.resolve_plan(&n.plan)?;
            let (sw, sh) = (n.plan.out_w, n.plan.out_h);
            // The REEL tier above the chain input (R.58, item 459): the sub-composite is
            // assembled larger than this instance’s chain can
            // consume. We fit it in BEFORE the chain: `Resample` to the whole
            // frame is the mirror of the CPU `fit_into` (the same bilinear as the
            // dry side of `FxOp::Mix`).
            let fit = n.fit_to(src.src_scale, ow, oh);
            let (cw, ch) = fit.unwrap_or((sw, sh));
            let (mut ops, (fw, fh), ha, ha_known, (mw, mh)) =
                // the sub-composite always has alpha (see below)
                chain_ops(&src.fx, cw, ch, src.src_scale, ow, oh, src.u, t, src.cam3d, src.lead, true, &src.masks);
            if fit.is_some() {
                ops.insert(
                    0,
                    FxOp::Resample { tw: cw, th: ch, frame: (0.0, 0.0, sw as f64, sh as f64) },
                );
            }
            if mw > self.max_dim || mh > self.max_dim {
                return Err(GpuExecError::TooLarge { w: mw, h: mh });
            }
            // the sub-composite always has alpha: where there are no layers, it is transparent
            let has_alpha = if ha_known { ha } else { true };
            return Ok(Some((
                ChainSpec { ops, src: ChainSrc::Nest(jobs), src_w: sw, src_h: sh, has_alpha, src_has_alpha: true },
                (fw, fh) == (ow, oh),
                sub_ok,
            )));
        }
        let Some(frame) = src.input.frame() else { return Ok(None) };
        if frame.width > self.max_dim || frame.height > self.max_dim {
            return Err(GpuExecError::TooLarge { w: frame.width, h: frame.height });
        }
        let (ops, (fw, fh), ha, ha_known, (mw, mh)) = chain_ops(
            &src.fx,
            frame.width,
            frame.height,
            src.src_scale,
            ow,
            oh,
            src.u,
            t,
            src.cam3d,
            src.lead,
            frame.has_alpha,
            &src.masks,
        );
        // The K×-gaps of the plan (item 314) MUST fit within the device texture limit;
        // if they do not, a clean TooLarge (the whole plan goes to the
        // CPU, which computes THE SAME plan, so the semantics are one)
        if mw > self.max_dim || mh > self.max_dim {
            return Err(GpuExecError::TooLarge { w: mw, h: mh });
        }
        let has_alpha = if ha_known { ha } else { frame.has_alpha };
        let view = self.upload.get_or_upload(&self.device, &self.queue, src.content, frame);
        let fits = (fw, fh) == (ow, oh);
        Ok(Some((
            ChainSpec {
                ops,
                src: ChainSrc::Tex(view),
                src_w: frame.width,
                src_h: frame.height,
                has_alpha,
                src_has_alpha: frame.has_alpha,
            },
            fits,
            true,
        )))
    }

    /// Encodes the layer chain; returns the view of the result plus the transients,
    /// which cannot be returned to the pool before the composite pass.
    fn encode_chain(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        chain: &ChainSpec,
    ) -> (wgpu::TextureView, Vec<Tex>) {
        let mut owned: Option<Tex> = None;
        let mut retired: Vec<Tex> = Vec::new();
        let (mut w, mut h) = (chain.src_w, chain.src_h);
        // The chain INPUT: a frame from the cache or the sub-composite of an embedding (R.50).
        let mut view = match &chain.src {
            ChainSrc::Tex(v) => v.clone(),
            ChainSrc::Nest(jobs) => {
                let sub = self.encode_jobs(enc, jobs, w, h);
                // THE EMBEDDING BOUNDARY: the composite accumulator is premultiplied
                // while the alpha is straight: exactly one conversion, a mirror of
                // `kt_engine::video::unpremultiply` (shaders/unpremul.wgsl).
                let straight = self.pool.acquire(&self.device, w, h);
                let dummy = self.dummy.view.clone();
                self.pass(
                    enc,
                    Pipe::Unpremul,
                    &straight.view,
                    &sub.view,
                    &dummy,
                    &dummy,
                    UniformData::default(),
                );
                let v = straight.view.clone();
                self.pool.release(sub);
                retired.push(straight);
                v
            }
        };
        let dummy = self.dummy.view.clone();
        // the dry side of the mix (R.38 item 12): the snapshot of the state BEFORE the contribution
        // of the layer. The snapshot texture cannot go back to the pool before the mix itself.
        let mut snap: Option<(wgpu::TextureView, u32, u32, bool)> = None;
        // the dry side of an effect slot's masks (shape-masks), kept apart from the morpher's
        let mut msnap: Option<(wgpu::TextureView, u32, u32)> = None;
        for op in &chain.ops {
            match op {
                FxOp::Snapshot { alpha } => {
                    let dry_alpha = alpha.unwrap_or(chain.src_has_alpha);
                    // the current result still lives in `owned` (or it is the source
                    // itself): we hold it until the end of the chain
                    if let Some(t) = owned.take() {
                        snap = Some((t.view.clone(), w, h, dry_alpha));
                        retired.push(t);
                    } else {
                        snap = Some((view.clone(), w, h, dry_alpha));
                    }
                }
                FxOp::MaskSnapshot => {
                    if let Some(t) = owned.take() {
                        msnap = Some((t.view.clone(), w, h));
                        retired.push(t);
                    } else {
                        msnap = Some((view.clone(), w, h));
                    }
                }
                FxOp::MaskAlpha { cov, key, has_alpha } => {
                    let cov_view = self.upload.get_or_upload(&self.device, &self.queue, *key, cov);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData { u: [*has_alpha as u32, 0, 0, 0, 0, 0, 0, 0], ..Default::default() };
                    self.pass(enc, Pipe::MaskAlpha, &dst.view, &view, &cov_view, &dummy, u);
                    if let Some(t) = owned.take() {
                        retired.push(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::MaskMix { cov, key, dry_alpha, wet_alpha } => {
                    let Some((dry_view, dw, dh)) = msnap.take() else { continue };
                    let dry_fit = if (dw, dh) == (w, h) {
                        dry_view
                    } else {
                        let fit = self.pool.acquire(&self.device, w, h);
                        let u = UniformData {
                            u: [w, h, dw, dh, 0, 0, 0, 0],
                            f: [0.0, 0.0, dw as f32, dh as f32, 0.0, 0.0, 0.0, 0.0],
                            ..Default::default()
                        };
                        self.pass(enc, Pipe::Resample, &fit.view, &dry_view, &dummy, &dummy, u);
                        let v = fit.view.clone();
                        retired.push(fit);
                        v
                    };
                    let cov_view = self.upload.get_or_upload(&self.device, &self.queue, *key, cov);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [0, *dry_alpha as u32, *wet_alpha as u32, 1, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::LayerMix, &dst.view, &dry_fit, &view, &cov_view, u);
                    if let Some(t) = owned.take() {
                        retired.push(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Mix { k, alpha } => {
                    let Some((dry_view, dw, dh, dry_alpha)) = snap.clone() else { continue };
                    let wet_alpha = alpha.unwrap_or(chain.src_has_alpha);
                    // the sizes may have diverged: we fit the dry side in
                    // THE SAME bilinear as on the CPU (fit_into)
                    let dry_fit = if (dw, dh) == (w, h) {
                        dry_view
                    } else {
                        let fit = self.pool.acquire(&self.device, w, h);
                        let u = UniformData {
                            u: [w, h, dw, dh, 0, 0, 0, 0],
                            f: [0.0, 0.0, dw as f32, dh as f32, 0.0, 0.0, 0.0, 0.0],
                            ..Default::default()
                        };
                        self.pass(enc, Pipe::Resample, &fit.view, &dry_view, &dummy, &dummy, u);
                        let v = fit.view.clone();
                        retired.push(fit);
                        v
                    };
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*k, dry_alpha as u32, wet_alpha as u32, 0, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::LayerMix, &dst.view, &dry_fit, &view, &dummy, u);
                    if let Some(t) = owned.take() {
                        retired.push(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Resample { tw, th, frame } => {
                    let dst = self.pool.acquire(&self.device, *tw, *th);
                    let u = UniformData {
                        u: [*tw, *th, w, h, 0, 0, 0, 0],
                        f: [
                            frame.0 as f32,
                            frame.1 as f32,
                            frame.2 as f32,
                            frame.3 as f32,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                        ],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    w = *tw;
                    h = *th;
                    owned = Some(dst);
                }
                FxOp::Mirror { h: fh, v: fv } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *fh as u32, *fv as u32, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Mirror, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::PluginBarrier { base, nodes } => {
                    use kt_plugin_api::barrier::{Node, Slot};
                    // `src` — the barrier's input, `prev` — the last node's output, `saved` — kept ones;
                    // textures the graph made are released when nothing reads them any more
                    let src = view.clone();
                    let (mut pw, mut ph) = (w, h);
                    let mut prev = view.clone();
                    let mut prev_tex: Option<Tex> = None;
                    let mut saved: Vec<(u8, wgpu::TextureView, Option<Tex>)> = Vec::new();
                    for n in nodes.iter() {
                        let pick = |s: &Slot, prev: &wgpu::TextureView| -> wgpu::TextureView {
                            match s {
                                Slot::Src => src.clone(),
                                Slot::Prev => prev.clone(),
                                Slot::Saved(i) => saved.iter().find(|(k, ..)| k == i).expect("checked graph").1.clone(),
                                Slot::None => dummy.clone(),
                            }
                        };
                        let (tw, th, which, t0, t1, t2, u) = match n {
                            Node::Save(i) => {
                                saved.push((*i, prev.clone(), prev_tex.take()));
                                continue;
                            }
                            Node::Premul => (pw, ph, Which::Fixed(Pipe::Premul), prev.clone(), dummy.clone(), dummy.clone(), UniformData::default()),
                            Node::Unpremul => (pw, ph, Which::Fixed(Pipe::Unpremul), prev.clone(), dummy.clone(), dummy.clone(), UniformData::default()),
                            Node::Resample { w: rw, h: rh } => (
                                *rw,
                                *rh,
                                Which::Fixed(Pipe::Resample),
                                prev.clone(),
                                dummy.clone(),
                                dummy.clone(),
                                UniformData {
                                    u: [*rw, *rh, pw, ph, 0, 0, 0, 0],
                                    f: [0.0, 0.0, pw as f32, ph as f32, 0.0, 0.0, 0.0, 0.0],
                                    ..Default::default()
                                },
                            ),
                            Node::Pass { shader, inputs, u, f } | Node::PassAt { shader, inputs, u, f, .. } => (
                                match n {
                                    Node::PassAt { w: tw, .. } => *tw,
                                    _ => w,
                                },
                                match n {
                                    Node::PassAt { h: th, .. } => *th,
                                    _ => h,
                                },
                                Which::Plugin(*base + *shader as usize),
                                pick(&inputs[0], &prev),
                                pick(&inputs[1], &prev),
                                pick(&inputs[2], &prev),
                                UniformData { u: *u, f: *f, ..Default::default() },
                            ),
                        };
                        let dst = self.pool.acquire(&self.device, tw, th);
                        self.pass_which(enc, which, &dst.view, &t0, &t1, &t2, u);
                        if let Some(t) = prev_tex.take() {
                            self.pool.release(t);
                        }
                        prev = dst.view.clone();
                        prev_tex = Some(dst);
                        pw = tw;
                        ph = th;
                    }
                    debug_assert_eq!((pw, ph), (w, h), "a barrier graph ends off the frame size");
                    for (_, _, t) in saved {
                        if let Some(t) = t {
                            self.pool.release(t);
                        }
                    }
                    if let Some(t) = prev_tex {
                        if let Some(o) = owned.take() {
                            self.pool.release(o);
                        }
                        view = prev;
                        owned = Some(t);
                    }
                }
                FxOp::PluginTap { idx, f, positioned } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let mut f = *f;
                    if *positioned {
                        // the barrier's stage is the pass's frame (D11.1)
                        f[6] = w as f32;
                        f[7] = h as f32;
                    }
                    let u = UniformData { f, ..Default::default() };
                    self.pass_which(enc, Which::Plugin(*idx), &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::GeomFused { prog } => {
                    debug_assert_eq!((prog.in_w, prog.in_h), (w, h), "geom run input dims");
                    let dst = self.pool.acquire(&self.device, prog.out_w, prog.out_h);
                    let u = encode_geom(prog);
                    self.pass_geom(enc, &dst.view, &view, &u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    w = prog.out_w;
                    h = prog.out_h;
                    owned = Some(dst);
                }
                FxOp::GeomTile { prog, tile } => {
                    debug_assert_eq!((prog.in_w, prog.in_h), (w, h), "geom run input dims");
                    let spec = prog.tile.expect("a tile op carries a marked program");
                    let cell = self.pool.acquire(&self.device, tile.out_w, tile.out_h);
                    let u = encode_geom(tile);
                    self.pass_geom(enc, &cell.view, &view, &u);
                    let dst = self.pool.acquire(&self.device, prog.out_w, prog.out_h);
                    let u = UniformData {
                        u: [spec.offx, spec.offy, spec.cw, spec.ch, spec.cols, spec.rows, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::GridTile, &dst.view, &cell.view, &dummy, &dummy, u);
                    self.pool.release(cell);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    w = prog.out_w;
                    h = prog.out_h;
                    owned = Some(dst);
                }
            }
        }
        if let Some(t) = owned {
            retired.push(t);
        }
        (view, retired)
    }

    /// The merged geometry-series pass: GEOM_UNIFORM_BYTES uniform (4 arena slots),
    /// its own layout; the rest of the mechanics is as in `pass`.
    fn pass_geom(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        t0: &wgpu::TextureView,
        u: &GeomUniform,
    ) {
        let offset = self.slot as u64 * SLOT;
        debug_assert!(
            self.slot + GEOM_SLOT_UNITS <= self.arena_slots,
            "uniform arena overflow (geom)"
        );
        self.slot += GEOM_SLOT_UNITS;
        self.queue.write_buffer(&self.arena, offset, &u.bytes());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.geom_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.arena,
                        offset: 0,
                        size: wgpu::BufferSize::new(GEOM_UNIFORM_BYTES as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(t0),
                },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&self.geom_pipe);
        rp.set_bind_group(0, &bg, &[offset as u32]);
        rp.draw(0..3, 0..1);
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        pipe: Pipe,
        target: &wgpu::TextureView,
        t0: &wgpu::TextureView,
        t1: &wgpu::TextureView,
        t2: &wgpu::TextureView,
        u: UniformData,
    ) {
        self.pass_which(enc, Which::Fixed(pipe), target, t0, t1, t2, u)
    }

    /// `pass` over a fixed pipe or an assembled plugin pipe (plugin-api-stage1, D5).
    #[allow(clippy::too_many_arguments)]
    fn pass_which(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        which: Which,
        target: &wgpu::TextureView,
        t0: &wgpu::TextureView,
        t1: &wgpu::TextureView,
        t2: &wgpu::TextureView,
        u: UniformData,
    ) {
        let offset = self.slot as u64 * SLOT;
        debug_assert!(self.slot < self.arena_slots, "uniform arena overflow");
        self.slot += 1;
        self.queue.write_buffer(&self.arena, offset, &u.bytes());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.arena,
                        offset: 0,
                        size: wgpu::BufferSize::new(128),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(t0),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(t1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(t2),
                },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        match which {
            Which::Fixed(pipe) => rp.set_pipeline(&self.pipes[pipe as usize]),
            Which::Plugin(i) => rp.set_pipeline(&self.plugin_pipes[i]),
        }
        rp.set_bind_group(0, &bg, &[offset as u32]);
        rp.draw(0..3, 0..1);
    }
}

/// ReadbackCache key: the full description of the plan. The frame is addressed by the SERIAL
/// NUMBER of the content (R.53, item 409), not by the pair (source, cache_idx):
/// by address the memo would return the old composite after an edit of the generator’s parameters
/// with a still cursor.
fn put_lead(h: &mut xxhash_rust::xxh3::Xxh3, lead: &Option<kt_fx::conform::LeadCtx>) {
    match lead {
        None => h.update(&[0u8]),
        Some(l) => {
            h.update(&[1u8, l.rule as u8]);
            // the raster moves the "as is" frame (still-image-frame-rule): it is part of the plan
            for v in [l.view, l.raster] {
                match v {
                    None => h.update(&[0u8]),
                    Some((w, hh)) => {
                        h.update(&[1u8]);
                        h.update(&w.to_bits().to_le_bytes());
                        h.update(&hh.to_bits().to_le_bytes());
                    }
                }
            }
        }
    }
}

fn plan_key(plan: &FramePlan<'_>) -> u64 {
    fn put_src(h: &mut Xxh3, s: &LayerSrc<'_>) {
        h.update(&s.source.0.to_le_bytes());
        h.update(&s.cache_idx.to_le_bytes());
        h.update(&s.content.to_le_bytes());
        // frame blending (R.37 S5) goes INTO THE KEY: the same two frames with another
        // weight give a DIFFERENT picture, and without this ReadbackCache would return
        // someone else’s mix (the same reason as put_mix)
        match &s.blend {
            None => h.update(&[0u8]),
            Some(b) => {
                h.update(&[1u8]);
                h.update(&b.cache_idx.to_le_bytes());
                h.update(&b.k.to_le_bytes());
                h.update(&b.content.to_le_bytes());
            }
        }
        h.update(&s.fx_hash.to_le_bytes());
        // the masks evaluated at THIS frame (shape-masks): an animated mask on a still frame
        // must not return another frame's composite; maskless layers add nothing
        if !s.masks.is_empty() {
            h.update(format!("{:?}", s.masks).as_bytes());
        }
        // decode scale step (R.46, item 318): a frame of the same key in another
        // size gives another picture; the plan MUST differ
        h.update(&s.src_scale.to_le_bytes());
        h.update(&s.u.to_bits().to_le_bytes());
        put_cam3d(h, &s.cam3d);
        // context of the leading “Geometry” node (R.62): the same frame with the same
        // chain but another rule/view gives another picture
        put_lead(h, &s.lead);
        match &s.input {
            kt_engine::video_plan::LayerInput::Frame(None) => h.update(&[0u8]),
            kt_engine::video_plan::LayerInput::Frame(Some(f)) => {
                h.update(&[1u8, f.has_alpha as u8]);
                h.update(&f.width.to_le_bytes());
                h.update(&f.height.to_le_bytes());
            }
            // The subtree goes ENTIRELY INTO THE KEY (R.50): a reel has neither a frame nor a
            // `cache_idx` by which it could be told apart, while
            // ReadbackCache MUST distinguish different sub-composites.
            kt_engine::video_plan::LayerInput::Nest(n) => {
                h.update(&[2u8]);
                h.update(&n.seq.0.to_le_bytes());
                // content frame mode (R.56, item 448): the same reel at the
                // same time in “blend” and in “nearest” gives DIFFERENT
                // pictures, and the key, not luck, MUST tell them apart
                h.update(&[n.blend as u8]);
                // REEL tier (R.58, item 459): with it, above the chain input
                // a fit is placed before the chain, and the bytes differ
                h.update(&n.nest_scale.to_le_bytes());
                h.update(&n.content_hash.to_le_bytes());
                h.update(&n.inner_t.0.to_le_bytes());
                put_plan(h, &n.plan);
            }
        }
    }
    /// Transition parameters go INTO THE KEY: without them ReadbackCache would return a frame of
    /// another transition kind (R.33 S4). The ext scalars of volumetric kinds (R.51,
    /// `trans_ext`) are NOT included here and must not be: this is a deterministic
    /// function of the already-hashed (kind, p, k), so there is nothing “missing” here. A plugin
    /// kind is hashed by its id, not by its mode: the mode is a function of the registry's order (D12).
    fn put_mix(h: &mut Xxh3, m: &TransMix) {
        match kt_engine::transitions::plugin_id(m.mode) {
            Some(id) => {
                h.update(&[1u8]);
                h.update(id.as_bytes());
                h.update(&[0u8]);
            }
            None => h.update(&m.mode.to_le_bytes()),
        }
        for v in m.p {
            h.update(&v.to_bits().to_le_bytes());
        }
    }
    fn put_plan(h: &mut Xxh3, plan: &FramePlan<'_>) {
        h.update(&plan.t.0.to_le_bytes());
        h.update(&plan.out_w.to_le_bytes());
        h.update(&plan.out_h.to_le_bytes());
        for layer in &plan.layers {
            match layer {
                PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                    h.update(&[1u8, *scale_alpha as u8]);
                    h.update(&alpha_k.to_le_bytes());
                    match fade {
                        None => h.update(&[0u8]),
                        Some(f) => {
                            h.update(&[1u8, f.incoming as u8]);
                            h.update(&f.k.to_le_bytes());
                            put_mix(h, &f.mix);
                        }
                    }
                    put_src(h, src);
                }
                PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                    h.update(&[2u8]);
                    h.update(&k.to_le_bytes());
                    h.update(&alpha_k.to_le_bytes());
                    put_mix(h, mix);
                    put_src(h, a);
                    put_src(h, b);
                }
            }
        }
    }
    let mut h = Xxh3::new();
    put_plan(&mut h, plan);
    h.digest()
}
