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
            GeomPrim::Warp { mode, p } => {
                u.p[cur] = [*mode as f32, p[0] as f32, p[1] as f32, p[2] as f32];
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
            kt_fx::geom::TapKind::Gs { key, tol, hard, spill, kc } => {
                let meta = (op.after_prim as u32) << 4;
                u.p[base] = [f32::from_bits(meta), key[0] as f32, key[1] as f32, key[2] as f32];
                u.p[base + 1] = [tol as f32, hard as f32, spill as f32, kc as f32];
            }
            kt_fx::geom::TapKind::Noise { seed, k, cell, color, special, stage_w, stage_h } => {
                let meta = 1u32
                    | ((op.after_prim as u32) << 4)
                    | ((color as u32) << 9)
                    | ((special as u32) << 10);
                u.p[base] = [
                    f32::from_bits(meta),
                    f32::from_bits(seed),
                    f32::from_bits(k as u32),
                    cell as f32,
                ];
                u.p[base + 1] = [stage_w as f32, stage_h as f32, 0.0, 0.0];
            }
            kt_fx::geom::TapKind::Grade { brightness, contrast, saturation, hue_cos, hue_sin } => {
                let meta = 2u32 | ((op.after_prim as u32) << 4);
                u.p[base] =
                    [f32::from_bits(meta), brightness as f32, contrast as f32, saturation as f32];
                u.p[base + 1] = [hue_cos as f32, hue_sin as f32, 0.0, 0.0];
            }
            kt_fx::geom::TapKind::Duo { shadow, high, amount } => {
                let meta = 3u32 | ((op.after_prim as u32) << 4);
                u.p[base] =
                    [f32::from_bits(meta), shadow[0] as f32, shadow[1] as f32, shadow[2] as f32];
                u.p[base + 1] = [high[0] as f32, high[1] as f32, high[2] as f32, amount as f32];
            }
            kt_fx::geom::TapKind::Poster { levels } => {
                let meta = 4u32 | ((op.after_prim as u32) << 4);
                u.p[base] = [f32::from_bits(meta), levels as f32, 0.0, 0.0];
                u.p[base + 1] = [0.0; 4];
            }
            kt_fx::geom::TapKind::Vig { strength, radius, softness, stage_w, stage_h } => {
                let meta = 5u32 | ((op.after_prim as u32) << 4);
                u.p[base] = [f32::from_bits(meta), strength as f32, radius as f32, softness as f32];
                u.p[base + 1] = [stage_w as f32, stage_h as f32, 0.0, 0.0];
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
    Noise,
    Greenscreen,
    Edges,
    Vhs,
    GlowAdd,
    Composite,
    /// Layer blend strength (R.38 item 12): mixing of the dry and wet sides.
    LayerMix,
    // pointwise R.47 in the barrier position
    ColorGrade,
    Duotone,
    Posterize,
    Vignette,
    // R.47 barriers Kv3
    SharpenAdd,
    Pixelate,
    ChromaAb,
    RadialBlur,
    ShadowUnder,
    Halftone,
    /// Embedding boundary (R.50): straight alpha from premultiplied.
    Unpremul,
    /// Premultiplication before the mip (R.53, item 418).
    Premul,
}

const PIPE_COUNT: usize = 21;

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
    Noise {
        seed: u32,
        k: u32,
        size: u32,
        color: bool,
        res_scale: u32,
    },
    Greenscreen {
        key: [f32; 3],
        tol: f32,
        hard: f32,
        spill: f32,
        kc: u32,
    },
    Edges {
        th: u32,
        d: u32,
        color: [u32; 3],
    },
    Vhs {
        shift: u32,
        line_off: u32,
        bleed: f32,
        jq: i32,
        step: u32,
        seed: u32,
        res_scale: u32,
    },
    /// Bloom: downscale dw×dh → upscale → additive ×s (3 passes).
    Glow {
        dw: u32,
        dh: u32,
        s: f32,
        th: u32,
        tint: [f32; 3],
        /// Mipping of PREMULTIPLIED data (R.53, item 418).
        pm: bool,
    },
    /// Premultiplication/reverse conversion around the mip (R.53, item 418).
    Premul,
    Unpremul,
    /// R.47 barriers Kv3. Sharpen/DropShadow take 3 passes (mip down/up +
    /// final), the others take one.
    Sharpen {
        dw: u32,
        dh: u32,
        amount: f32,
    },
    Pixelate {
        cell: u32,
    },
    ChromaAb {
        amount: f32,
    },
    RadialBlur {
        amount: f32,
        cx: f32,
        cy: f32,
        spin: u32,
    },
    DropShadow {
        dw: u32,
        dh: u32,
        ox: i32,
        oy: i32,
        opacity: f32,
        tint: [f32; 3],
    },
    Halftone {
        size: f32,
        ca: f32,
        sa: f32,
        k: f32,
    },
    /// Pointwise R.47 at the barrier position: one pass, mirrors of tap_*.
    ColorGrade {
        br: f32,
        con: f32,
        sat: f32,
        hc: f32,
        hs: f32,
    },
    Duotone {
        shadow: [f32; 3],
        high: [f32; 3],
        amount: f32,
    },
    Posterize {
        levels: u32,
    },
    Vignette {
        strength: f32,
        radius: f32,
        softness: f32,
    },
    /// Merged geometry series (R.44): the program of inverse mappings is built in
    /// kt-fx (shared code with the CPU executor), one pass, a 1024 B uniform.
    /// EventGrid always takes this path (the mosaic of the former 2 passes is removed).
    GeomFused {
        prog: kt_fx::geom::GeomProgram,
    },
    /// Snapshot of the state BEFORE a layer’s contribution (R.38 item 12): the dry side
    /// of the future mix. A marker only; it spawns no passes.
    Snapshot,
    /// Mix the current result with the snapshot with weight `k` (0..4096).
    /// The sizes could have diverged (Pan/CamMotion resample), in which case the dry side is fitted
    /// by THE SAME bilinear as on the CPU.
    Mix {
        k: u32,
    },
}

impl FxOp {
    fn passes(&self) -> u32 {
        match self {
            FxOp::Glow { .. } => 3,
            FxOp::Sharpen { .. } | FxOp::DropShadow { .. } => 3,
            // a snapshot spawns no passes; when the sizes have diverged, mixing is completed
            // by resampling the dry side
            FxOp::Snapshot => 0,
            FxOp::Mix { .. } => 2,
            _ => 1,
        }
    }

    /// Uniform arena slots (SLOT = 256 B): an ordinary pass takes one slot,
    /// a merged geometry pass takes GEOM_SLOT_UNITS. The sum MUST predict EXACTLY
    /// the consumption of the arena (debug_assert in pass/pass_geom).
    fn slot_units(&self) -> u32 {
        match self {
            FxOp::GeomFused { .. } => GEOM_SLOT_UNITS,
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
    let plan = kt_fx::plan::plan_chain_in(
        fx, src_w, src_h, src_scale, out_w, out_h, u, cam3d, t.0, &froms, lead,
    );
    for op in plan.ops {
        match op {
            kt_fx::plan::PlanOp::Snapshot => ops.push(FxOp::Snapshot),
            kt_fx::plan::PlanOp::Mix { idx } => {
                if let FxInstance::LayerMix { amount, .. } = &fx[idx] {
                    let k = (amount.clamp(0.0, 1.0) * 4096.0).round() as u32;
                    if k < 4096 {
                        ops.push(FxOp::Mix { k });
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
                    FxInstance::Mirror { horizontal, vertical } => {
                        if *horizontal || *vertical {
                            ops.push(FxOp::Mirror { h: *horizontal, v: *vertical });
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
                    ops.push(FxOp::GeomFused { prog });
                }
            },
            kt_fx::plan::PlanOp::Barrier { idx, res_scale } => match &fx[idx] {
                FxInstance::Blur { radius, radius_y } => {
                    // R.45 (item 313): steps ×K: the mip is the same, the view is invariant
                    let radius = (*radius).clamp(1, 48);
                    let step = ((radius / 3).max(1) + 1) * res_scale;
                    let step_y = if *radius_y == 0 {
                        step
                    } else {
                        (((*radius_y).clamp(1, 48) / 3).max(1) + 1) * res_scale
                    };
                    let (dw, dh) = ((w / step).max(1), (h / step_y).max(1));
                    // ALPHA RULE (item 418): the mip runs over the
                    // premultiplied data: a mirror of the gate in
                    // `kt_fx::video::blur`. When a == 255 both conversions
                    // are identities, so the passes are not set up at all.
                    let pm = if has_alpha_known { has_alpha } else { src_has_alpha };
                    if pm {
                        ops.push(FxOp::Premul);
                    }
                    ops.push(FxOp::Resample {
                        tw: dw,
                        th: dh,
                        frame: (0.0, 0.0, w as f64, h as f64),
                    });
                    ops.push(FxOp::Resample {
                        tw: w,
                        th: h,
                        frame: (0.0, 0.0, dw as f64, dh as f64),
                    });
                    if pm {
                        ops.push(FxOp::Unpremul);
                    }
                }
                FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
                    let tol = (*tolerance).clamp(1, 200);
                    let soft = (*softness).clamp(0, 100);
                    let hard = (tol as f64 - soft as f64).max(0.0);
                    let key = [*key_r, *key_g, *key_b];
                    ops.push(FxOp::Greenscreen {
                        key: [key[0] as f32, key[1] as f32, key[2] as f32],
                        tol: tol as f32,
                        hard: hard as f32,
                        spill: spill.clamp(0.0, 1.0) as f32,
                        kc: (0..3).max_by_key(|&i| key[i]).unwrap_or(1) as u32,
                    });
                    has_alpha_known = true;
                    has_alpha = true;
                }
                FxInstance::Edges { threshold, thickness, color_r, color_g, color_b } => {
                    ops.push(FxOp::Edges {
                        th: (*threshold).clamp(1, 255),
                        // R.45 (item 313): Sobel stride ×K
                        d: (*thickness).clamp(1, 8) * res_scale,
                        color: [*color_r as u32, *color_g as u32, *color_b as u32],
                    });
                    // ALPHA RULE (R.53, item 418): edges SKIPS the alpha
                    // (`a_out = max(a_in, line strength)`) rather than destroying
                    // it: it does not touch the convolution, like all the geometry.
                }
                FxInstance::Noise { amount, size, color } => {
                    let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
                    if k > 0 {
                        ops.push(FxOp::Noise {
                            seed: kt_fx::video::noise_seed(t.0),
                            k: k as u32,
                            size: (*size).clamp(1, 64),
                            color: *color,
                            res_scale,
                        });
                    }
                }
                FxInstance::ColorGrade { brightness, contrast, saturation, hue } => {
                    let hr = (*hue as f64).to_radians();
                    ops.push(FxOp::ColorGrade {
                        br: *brightness,
                        con: *contrast,
                        sat: *saturation,
                        hc: hr.cos() as f32,
                        hs: hr.sin() as f32,
                    });
                }
                FxInstance::Duotone {
                    shadow_r,
                    shadow_g,
                    shadow_b,
                    high_r,
                    high_g,
                    high_b,
                    amount,
                } => {
                    ops.push(FxOp::Duotone {
                        shadow: [*shadow_r as f32, *shadow_g as f32, *shadow_b as f32],
                        high: [*high_r as f32, *high_g as f32, *high_b as f32],
                        amount: amount.clamp(0.0, 1.0),
                    });
                }
                FxInstance::Posterize { levels } => {
                    ops.push(FxOp::Posterize { levels: (*levels).clamp(2, 32) });
                }
                FxInstance::Vignette { strength, radius, softness } => {
                    ops.push(FxOp::Vignette {
                        strength: strength.clamp(0.0, 1.0),
                        radius: radius.clamp(0.0, 1.5),
                        softness: softness.clamp(0.05, 1.0),
                    });
                }
                FxInstance::Sharpen { amount, radius } => {
                    let step = (*radius).clamp(2, 16) * res_scale;
                    let (dw, dh) = ((w / step).max(1), (h / step).max(1));
                    ops.push(FxOp::Sharpen { dw, dh, amount: amount.clamp(0.0, 4.0) });
                }
                FxInstance::Pixelate { size } => {
                    ops.push(FxOp::Pixelate { cell: (*size).clamp(2, 128) * res_scale });
                }
                FxInstance::ChromaticAberration { amount } => {
                    ops.push(FxOp::ChromaAb { amount: amount.clamp(0.0, 0.05) });
                }
                FxInstance::RadialBlur { amount, center_x, center_y, spin } => {
                    ops.push(FxOp::RadialBlur {
                        amount: amount.clamp(0.0, 1.0),
                        cx: 0.5 + center_x.clamp(-1.0, 1.0) * 0.5,
                        cy: 0.5 + center_y.clamp(-1.0, 1.0) * 0.5,
                        spin: *spin as u32,
                    });
                }
                FxInstance::DropShadow {
                    offset_x,
                    offset_y,
                    radius,
                    opacity,
                    tint_r,
                    tint_g,
                    tint_b,
                } => {
                    // opaque input: the CPU does a passthrough; the formula on
                    // alpha 255 degenerates into an identity, so the skip is
                    // an optimization, not a different result
                    if !(has_alpha_known && !has_alpha) {
                        let step = (*radius).clamp(1, 32) * res_scale;
                        let (dw, dh) = ((w / step).max(1), (h / step).max(1));
                        ops.push(FxOp::DropShadow {
                            dw,
                            dh,
                            ox: (*offset_x).clamp(-64, 64) * res_scale as i32,
                            oy: (*offset_y).clamp(-64, 64) * res_scale as i32,
                            opacity: opacity.clamp(0.0, 1.0),
                            tint: [*tint_r as f32, *tint_g as f32, *tint_b as f32],
                        });
                    }
                }
                FxInstance::Halftone { size, angle } => {
                    let a = (angle.clamp(-90.0, 90.0) as f64).to_radians();
                    ops.push(FxOp::Halftone {
                        size: (*size).clamp(2, 64) as f32,
                        ca: a.cos() as f32,
                        sa: a.sin() as f32,
                        k: res_scale as f32,
                    });
                }
                FxInstance::Vhs { rgb_shift, bleed, scan_step, jitter } => {
                    ops.push(FxOp::Vhs {
                        // R.45 (item 313): shift ×K; the line context (scan and
                        // jitter) the shader takes from the logical line y/K
                        shift: (*rgb_shift).clamp(0, 12) * res_scale,
                        line_off: ((t.0 / 3000) % 7) as u32,
                        bleed: bleed.clamp(0.0, 1.0) as f32,
                        step: (*scan_step).clamp(0, 32),
                        // the jitter quantum is the same as the CPU kernel computes
                        jq: (jitter.clamp(0.0, 1.0) * 1024.0).round() as i32,
                        seed: kt_fx::video::noise_seed(t.0),
                        res_scale,
                    });
                }
                FxInstance::Glow { strength, radius, threshold, tint_r, tint_g, tint_b } => {
                    let s = strength.clamp(0.0, 2.0) * 0.35;
                    if s > 0.0 {
                        // R.45 (item 313): mip divisor ×K — the mip is the same
                        let r = (*radius).clamp(1, 32) * res_scale;
                        ops.push(FxOp::Glow {
                            dw: (w / r).max(1),
                            dh: (h / r).max(1),
                            s: s as f32,
                            th: (*threshold).min(255),
                            tint: [*tint_r as f32, *tint_g as f32, *tint_b as f32],
                            // light from transparent places does not go (item 418):
                            // the premultiplied copy is mipped, the base of the addition
                            // stays straight
                            pm: if has_alpha_known { has_alpha } else { src_has_alpha },
                        });
                    }
                }
                _ => debug_assert!(false, "barrier op on non-barrier fx"),
            },
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

impl Job {
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
    /// The merged geometry pass (R.44): its own 1024 B uniform → its own layout/pipe.
    geom_layout: wgpu::BindGroupLayout,
    geom_pipe: wgpu::RenderPipeline,
    arena: wgpu::Buffer,
    arena_slots: u32,
    slot: u32,
    pool: TexturePool,
    upload: UploadCache,
    readback: ReadbackCache,
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
    ("noise", include_str!("shaders/noise.wgsl")),
    ("greenscreen", include_str!("shaders/greenscreen.wgsl")),
    ("edges", include_str!("shaders/edges.wgsl")),
    ("vhs", include_str!("shaders/vhs.wgsl")),
    ("glow_add", include_str!("shaders/glow_add.wgsl")),
    ("composite", include_str!("shaders/composite.wgsl")),
    ("layer_mix", include_str!("shaders/layer_mix.wgsl")),
    ("colorgrade", include_str!("shaders/colorgrade.wgsl")),
    ("duotone", include_str!("shaders/duotone.wgsl")),
    ("posterize", include_str!("shaders/posterize.wgsl")),
    ("vignette", include_str!("shaders/vignette.wgsl")),
    ("sharpen_add", include_str!("shaders/sharpen_add.wgsl")),
    ("pixelate", include_str!("shaders/pixelate.wgsl")),
    ("chroma_ab", include_str!("shaders/chroma_ab.wgsl")),
    ("radial_blur", include_str!("shaders/radial_blur.wgsl")),
    ("shadow_under", include_str!("shaders/shadow_under.wgsl")),
    ("halftone", include_str!("shaders/halftone.wgsl")),
    ("unpremul", include_str!("shaders/unpremul.wgsl")),
    ("premul", include_str!("shaders/premul.wgsl")),
];

const COMMON: &str = include_str!("shaders/common.wgsl");

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
        let pipes: Vec<wgpu::RenderPipeline> = SHADERS
            .iter()
            .map(|(name, body)| {
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
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/geom_fused.wgsl").into()),
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
                geom_layout,
                geom_pipe,
                arena,
                arena_slots,
                slot: 0,
                pool: TexturePool::new(TRANSIENT_BUDGET),
                upload: UploadCache::new(UPLOAD_BUDGET),
                readback: ReadbackCache::new(READBACK_BUDGET),
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
}

impl Inner {
    fn execute(&mut self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        let (ow, oh) = (plan.out_w, plan.out_h);
        if ow > self.max_dim || oh > self.max_dim {
            return Err(GpuExecError::TooLarge { w: ow, h: oh });
        }
        let key = plan_key(plan);
        if let Some((data, complete)) = self.readback.get(key) {
            out.clear();
            out.extend_from_slice(data);
            return Ok(complete);
        }

        // --- resolve: presence/size by the rules of the CPU executor ---
        let (jobs, complete) = self.resolve_plan(plan)?;

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
                chain_ops(&src.fx, cw, ch, src.src_scale, ow, oh, src.u, t, src.cam3d, src.lead, true);
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
                ChainSpec { ops, src: ChainSrc::Nest(jobs), src_w: sw, src_h: sh, has_alpha },
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
        let mut snap: Option<(wgpu::TextureView, u32, u32)> = None;
        for op in &chain.ops {
            match op {
                FxOp::Snapshot => {
                    // the current result still lives in `owned` (or it is the source
                    // itself): we hold it until the end of the chain
                    if let Some(t) = owned.take() {
                        snap = Some((t.view.clone(), w, h));
                        retired.push(t);
                    } else {
                        snap = Some((view.clone(), w, h));
                    }
                }
                FxOp::Mix { k } => {
                    let Some((dry_view, dw, dh)) = snap.clone() else { continue };
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
                        u: [*k, chain.has_alpha as u32, chain.has_alpha as u32, 0, 0, 0, 0, 0],
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
                FxOp::Noise { seed, k, size, color, res_scale } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*seed, *k, *size, *color as u32, *res_scale, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Noise, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Greenscreen { key, tol, hard, spill, kc } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*kc, 0, 0, 0, 0, 0, 0, 0],
                        f: [key[0], key[1], key[2], *tol, *hard, *spill, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Greenscreen, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Edges { th, d, color } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *th, *d, color[0], color[1], color[2], 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Edges, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Vhs { shift, line_off, bleed, jq, step, seed, res_scale } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, *shift, *line_off, *step, *seed, *jq as u32, *res_scale, 0],
                        f: [*bleed, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Vhs, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::ColorGrade { br, con, sat, hc, hs } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        f: [*br, *con, *sat, *hc, *hs, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::ColorGrade, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Duotone { shadow, high, amount } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        f: [
                            shadow[0], shadow[1], shadow[2], 0.0, high[0], high[1], high[2],
                            *amount,
                        ],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Duotone, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Posterize { levels } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        f: [*levels as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Posterize, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Vignette { strength, radius, softness } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, 0, 0, 0, 0, 0, 0],
                        f: [*strength, *radius, *softness, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Vignette, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Sharpen { dw, dh, amount } => {
                    let small = self.pool.acquire(&self.device, *dw, *dh);
                    let u = UniformData {
                        u: [*dw, *dh, w, h, 0, 0, 0, 0],
                        f: [0.0, 0.0, w as f32, h as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &small.view, &view, &dummy, &dummy, u);
                    let base = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *dw, *dh, 0, 0, 0, 0],
                        f: [0.0, 0.0, *dw as f32, *dh as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &base.view, &small.view, &dummy, &dummy, u);
                    self.pool.release(small);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        f: [*amount, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::SharpenAdd, &dst.view, &view, &base.view, &dummy, u);
                    self.pool.release(base);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Pixelate { cell } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData { u: [w, h, *cell, 0, 0, 0, 0, 0], ..Default::default() };
                    self.pass(enc, Pipe::Pixelate, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::ChromaAb { amount } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, 0, 0, 0, 0, 0, 0],
                        f: [*amount, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::ChromaAb, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::RadialBlur { amount, cx, cy, spin } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *spin, 0, 0, 0, 0, 0],
                        f: [*amount, *cx, *cy, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::RadialBlur, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::DropShadow { dw, dh, ox, oy, opacity, tint } => {
                    let small = self.pool.acquire(&self.device, *dw, *dh);
                    let u = UniformData {
                        u: [*dw, *dh, w, h, 0, 0, 0, 0],
                        f: [0.0, 0.0, w as f32, h as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &small.view, &view, &dummy, &dummy, u);
                    let blurred = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *dw, *dh, 0, 0, 0, 0],
                        f: [0.0, 0.0, *dw as f32, *dh as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &blurred.view, &small.view, &dummy, &dummy, u);
                    self.pool.release(small);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *ox as u32, *oy as u32, 0, 0, 0, 0],
                        f: [*opacity, tint[0], tint[1], tint[2], 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::ShadowUnder, &dst.view, &view, &blurred.view, &dummy, u);
                    self.pool.release(blurred);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Halftone { size, ca, sa, k } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, 0, 0, 0, 0, 0, 0],
                        f: [*size, *ca, *sa, *k, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Halftone, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Premul | FxOp::Unpremul => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let pipe =
                        if matches!(op, FxOp::Premul) { Pipe::Premul } else { Pipe::Unpremul };
                    self.pass(enc, pipe, &dst.view, &view, &dummy, &dummy, UniformData::default());
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Glow { dw, dh, s, th, tint, pm } => {
                    // the mip is taken from the premultiplied COPY; the base of
                    // the addition stays straight (a mirror of kt_fx::video::glow)
                    let pm_tex = pm.then(|| {
                        let t = self.pool.acquire(&self.device, w, h);
                        self.pass(
                            enc,
                            Pipe::Premul,
                            &t.view,
                            &view,
                            &dummy,
                            &dummy,
                            UniformData::default(),
                        );
                        t
                    });
                    let mip_src =
                        pm_tex.as_ref().map(|t| t.view.clone()).unwrap_or_else(|| view.clone());
                    let small = self.pool.acquire(&self.device, *dw, *dh);
                    let u = UniformData {
                        u: [*dw, *dh, w, h, 0, 0, 0, 0],
                        f: [0.0, 0.0, w as f32, h as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &small.view, &mip_src, &dummy, &dummy, u);
                    if let Some(t) = pm_tex {
                        self.pool.release(t);
                    }
                    let up = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *dw, *dh, 0, 0, 0, 0],
                        f: [0.0, 0.0, *dw as f32, *dh as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &up.view, &small.view, &dummy, &dummy, u);
                    self.pool.release(small);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*th, 0, 0, 0, 0, 0, 0, 0],
                        f: [*s, tint[0], tint[1], tint[2], 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::GlowAdd, &dst.view, &view, &up.view, &dummy, u);
                    self.pool.release(up);
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
        rp.set_pipeline(&self.pipes[pipe as usize]);
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
            match l.view {
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
    /// function of the already-hashed (mode, p, k), so there is nothing “missing” here.
    fn put_mix(h: &mut Xxh3, m: &TransMix) {
        h.update(&m.mode.to_le_bytes());
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
