//! Stylizing video effects as barrier plugins (plugin-api-barriers): pixelate, edges, halftone,
//! chromatic_aberration, vhs, noise. Pixelate is the pilot (D7): one pass, no SIMD twin, no neighbours read —
//! the pass graph's simplest form. Edges carries the family's AVX2 twins (`simd.rs`).

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::barrier::{noise_seed, BarrierKernel, GpuCtx, Node, Slot};
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, TapCtx, UiDecl, UiVariant,
    VideoClass, VideoKernel,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(Pixelate::new()),
        Arc::new(Edges::new()),
        Arc::new(Halftone::new()),
        Arc::new(ChromaticAberration::new()),
        Arc::new(Vhs::new()),
        Arc::new(Noise::new()),
    ]
}


/// Mosaic (R.47): an integer copy of the cell's centre, C = size·K — EXACT parity with the GPU by
/// construction.
pub struct Pixelate {
    m: Manifest,
}

impl Pixelate {
    pub fn new() -> Pixelate {
        Pixelate {
            m: Manifest {
                id: "pixelate".into(),
                media: Media::VideoFx,
                // the row the variant declared: size u32 2..128, default 12
                passport: vec![ParamSpec::num("size", 2.0, 128.0, 12.0, 1.0, ParamUnit::Px, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::U32, omit_default: false }],
                decl: Decl { video_class: Some(VideoClass::Barrier), gpu: Gpu::Barrier, ..Decl::default() },
                ui: UiDecl {
                    name: "Pixelate",
                    category_key: "fxreg.cat.style",
                    keywords_key: "fxreg.kw.pixelate",
                    variants: PIXELATE_VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

impl Default for Pixelate {
    fn default() -> Pixelate {
        Pixelate::new()
    }
}

const PIXELATE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.fine", sets: &[("size", 5.0)] },
    UiVariant { name_key: "fxreg.var.coarse", sets: &[("size", 28.0)] },
    UiVariant { name_key: "fxreg.var.censor", sets: &[("size", 64.0)] },
];

impl Plugin for Pixelate {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

impl BarrierKernel for Pixelate {
    /// The cell `size` clamped and cast once per frame (the variant held a `u32`).
    fn cook(&self, p: &[f64]) -> Cooked {
        let size = (p.first().copied().unwrap_or(12.0).round().max(0.0) as u32).clamp(2, 128);
        [size as f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("pixelate.wgsl")]
    }

    /// One pass: `u0 = (w, h, cell)`.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let cell = k[0] as u32 * g.res_scale;
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [g.w, g.h, cell, 0, 0, 0, 0, 0],
            f: [0.0; 8],
        });
    }
}

fn num(name: &'static str, min: f64, max: f64, default: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec::num(name, min, max, default, step, unit, ParamKind::Smooth)
}

fn row(store: Store, omit_default: bool) -> RowDecl {
    RowDecl { store, omit_default }
}

fn barrier_decl() -> Decl {
    Decl { video_class: Some(VideoClass::Barrier), gpu: Gpu::Barrier, ..Decl::default() }
}

macro_rules! plugin {
    ($t:ident) => {
        impl Default for $t {
            fn default() -> $t {
                $t::new()
            }
        }
        impl Plugin for $t {
            fn manifest(&self) -> &Manifest {
                &self.m
            }
            fn barrier(&self) -> Option<&dyn BarrierKernel> {
                Some(self)
            }
        }
    };
}

// ---- edges ----------------------------------------------------------------------------------

/// Sobel on luma: an outline of the given colour and thickness (the Sobel stride). The alpha rule
/// (R.53 item 418): edges RAISES alpha to the line's strength, `a_out = max(a_in, v)`, and an
/// opaque input stays opaque — it preserves.
pub struct Edges {
    m: Manifest,
}

impl Edges {
    pub fn new() -> Edges {
        let rgb = |n: &'static str| num(n, 0.0, 255.0, 255.0, 1.0, ParamUnit::Rgb8);
        Edges {
            m: Manifest {
                id: "edges".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("threshold", 1.0, 255.0, 48.0, 1.0, ParamUnit::None),
                    num("thickness", 1.0, 8.0, 1.0, 1.0, ParamUnit::Px),
                    rgb("color_r"),
                    rgb("color_g"),
                    rgb("color_b"),
                ],
                rows: vec![row(Store::U32, false), row(Store::U32, true), row(Store::U8, true), row(Store::U8, true), row(Store::U8, true)],
                decl: barrier_decl(),
                ui: UiDecl { name: "Edges", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.edges", variants: EDGES_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

const EDGES_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.ink", sets: &[("threshold", 90.0), ("color_r", 0.0), ("color_g", 0.0), ("color_b", 0.0)] },
    UiVariant {
        name_key: "fxreg.var.neon",
        sets: &[("threshold", 36.0), ("thickness", 2.0), ("color_r", 0.0), ("color_g", 255.0), ("color_b", 160.0)],
    },
    UiVariant { name_key: "fxreg.var.blueprint", sets: &[("threshold", 60.0), ("color_r", 80.0), ("color_g", 160.0), ("color_b", 255.0)] },
];

plugin!(Edges);

impl BarrierKernel for Edges {
    /// `[threshold, thickness, color_r, color_g, color_b]`, clamped.
    fn cook(&self, p: &[f64]) -> Cooked {
        [(p[0] as u32).clamp(1, 255) as f64, (p[1] as u32).clamp(1, 8) as f64, p[2], p[3], p[4], 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("edges.wgsl")]
    }

    /// One pass: `u` = (w, h, threshold, stride ×K, colour).
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [g.w, g.h, k[0] as u32, k[1] as u32 * g.res_scale, k[2] as u32, k[3] as u32, k[4] as u32, 0],
            f: [0.0; 8],
        });
    }
}

// ---- halftone -------------------------------------------------------------------------------

/// Halftone screen (R.47): black dots whose radius follows the cell's luma, on a grid rotated by the
/// angle; the pattern lives in LOGICAL coordinates l = p/K, so K× execution keeps the look.
pub struct Halftone {
    m: Manifest,
}

impl Halftone {
    pub fn new() -> Halftone {
        Halftone {
            m: Manifest {
                id: "halftone".into(),
                media: Media::VideoFx,
                passport: vec![num("size", 2.0, 64.0, 8.0, 1.0, ParamUnit::Px), num("angle", -90.0, 90.0, 45.0, 1.0, ParamUnit::Deg)],
                rows: vec![row(Store::U32, false), row(Store::F32, false)],
                decl: barrier_decl(),
                ui: UiDecl { name: "Halftone", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.halftone", variants: HALFTONE_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

const HALFTONE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.fine", sets: &[("size", 3.0), ("angle", 0.0)] },
    UiVariant { name_key: "fxreg.var.print", sets: &[("size", 5.0), ("angle", 15.0)] },
    UiVariant { name_key: "fxreg.var.comic", sets: &[("size", 12.0)] },
];

plugin!(Halftone);

impl BarrierKernel for Halftone {
    /// `[size, angle]`: the size clamped to 2..64, the angle (an f32 row) as held.
    fn cook(&self, p: &[f64]) -> Cooked {
        [(p[0] as u32).clamp(2, 64) as f64, p[1], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("halftone.wgsl")]
    }

    /// One pass: `f0` = (size, cos, sin, K) — the angle clamped in f32 and turned into radians in
    /// f64, as the GPU arm did before the move.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let a = ((k[1] as f32).clamp(-90.0, 90.0) as f64).to_radians();
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [g.w, g.h, 0, 0, 0, 0, 0, 0],
            f: [k[0] as f32, a.fcos() as f32, a.fsin() as f32, g.res_scale as f32, 0.0, 0.0, 0.0, 0.0],
        });
    }
}

// ---- chromatic aberration ---------------------------------------------------------------------

/// Chromatic aberration (R.47, F-FX-002): R sampled closer to the centre (×(1−a)), B further out
/// (×(1+a)), G and alpha in place. Positions are frame fractions, the K× execution scales itself.
pub struct ChromaticAberration {
    m: Manifest,
}

impl ChromaticAberration {
    pub fn new() -> ChromaticAberration {
        ChromaticAberration {
            m: Manifest {
                id: "chromatic_aberration".into(),
                media: Media::VideoFx,
                passport: vec![num("amount", 0.0, 0.05, 0.012, 0.001, ParamUnit::Norm)],
                rows: vec![row(Store::F32, false)],
                decl: barrier_decl(),
                ui: UiDecl {
                    name: "Chromatic Aberration",
                    category_key: "fxreg.cat.distort",
                    keywords_key: "fxreg.kw.chromatic_aberration",
                    variants: CHROMA_VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

const CHROMA_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.subtle", sets: &[("amount", 0.004)] },
    UiVariant { name_key: "fxreg.var.strong", sets: &[("amount", 0.028)] },
    // NOT 0.05 (the spec ceiling): the row is f32, and the clamp is computed in f64 — the exact
    // ceiling written to f32 rounds UP and lands OUTSIDE the spec (found by t543)
    UiVariant { name_key: "fxreg.var.glitch", sets: &[("amount", 0.045)] },
];

plugin!(ChromaticAberration);

impl BarrierKernel for ChromaticAberration {
    fn cook(&self, p: &[f64]) -> Cooked {
        [p[0].clamp(0.0, 0.05), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("chroma_ab.wgsl")]
    }

    /// One pass: `u0` = (w, h), `f0.x` = amount.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [g.w, g.h, 0, 0, 0, 0, 0, 0],
            f: [(k[0] as f32).clamp(0.0, 0.05), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        });
    }
}

// ---- noise ------------------------------------------------------------------------------------

/// Deterministic grain (R.29: a counter-based hash, so the WGSL gives the same bytes). A
/// DUAL-POSITION kind (design D4): inside a fused geometry series it is a positioned TAP (the f64
/// continuum, the grain pinned to its stage's grid, decision R.45); at a series edge it is the
/// integer BARRIER kernel with K× and its AVX2 twin. `size == 1` and no colour — the former formula
/// byte-for-byte on both.
pub struct Noise {
    m: Manifest,
}

impl Noise {
    pub fn new() -> Noise {
        Noise {
            m: Manifest {
                id: "noise".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("amount", 0.0, 1.0, 0.08, 0.01, ParamUnit::Norm),
                    num("size", 1.0, 64.0, 1.0, 1.0, ParamUnit::Px),
                    ParamSpec::flag_row("color", false),
                ],
                rows: vec![row(Store::F64, false), row(Store::U32, true), row(Store::Bool, true)],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("tap_noise.wgsl")),
                    positioned: true,
                    ..Decl::default()
                },
                ui: UiDecl { name: "Noise", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.noise", variants: NOISE_VARIANTS, ..UiDecl::default() },
            },
        }
    }

    /// k = ⌊amount·32⌋ (0 — no grain at all), the grain size 1..64 and the colour flag.
    fn grain(p: &[f64]) -> (i64, u32, bool) {
        ((p[0].clamp(0.0, 1.0) * 32.0) as i64, (p[1] as u32).clamp(1, 64), p[2] != 0.0)
    }
}

const NOISE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.film", sets: &[("amount", 0.05), ("size", 2.0)] },
    UiVariant { name_key: "fxreg.var.heavy", sets: &[("amount", 0.25)] },
    UiVariant { name_key: "fxreg.var.coarse", sets: &[("amount", 0.18), ("size", 8.0)] },
];

impl Default for Noise {
    fn default() -> Noise {
        Noise::new()
    }
}

impl Plugin for Noise {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

impl VideoKernel for Noise {
    /// `[seed, k, cell, colour, special, stage_w, stage_h]` — the seed from the frame's burst index.
    fn cook_at(&self, p: &[f64], cx: &TapCtx) -> Cooked {
        let (k, size, color) = Noise::grain(p);
        let special = size == 1 && !color;
        [noise_seed(cx.burst_index) as f64, k as f64, size as f64, color as u8 as f64, special as u8 as f64, cx.stage_w as f64, cx.stage_h as f64, 0.0]
    }
    /// k == 0 — the effect is empty (the kernel's early exit and the GPU branch): no tap at all.
    fn active(&self, k: &Cooked) -> bool {
        k[1] > 0.0
    }
    /// a.y = the seed's bits, a.z = k's bits, a.w = the cell, b.x = the flags; b.zw the host's.
    fn gpu_cooked(&self, k: &Cooked, _p: &[f64]) -> [f32; 7] {
        let flags = (k[3] != 0.0) as u32 | (((k[4] != 0.0) as u32) << 1);
        [f32::from_bits(k[0] as u32), f32::from_bits(k[1] as u32), k[2] as f32, flags as f32, 0.0, 0.0, 0.0]
    }
}

impl BarrierKernel for Noise {
    /// `[k, size, colour]`.
    fn cook(&self, p: &[f64]) -> Cooked {
        let (k, size, color) = Noise::grain(p);
        [k as f64, size as f64, color as u8 as f64, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("noise.wgsl")]
    }

    /// One pass when there is grain: `u` = (seed, k, size, colour, K).
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        if k[0] <= 0.0 {
            return;
        }
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [noise_seed(g.burst_index), k[0] as u32, k[1] as u32, (k[2] != 0.0) as u32, g.res_scale, 0, 0, 0],
            f: [0.0; 8],
        });
    }
}

// ---- vhs --------------------------------------------------------------------------------------

/// VHS: RGB shift + scan lines (v1: lines every 4, jitter from time). R.45 (item 313): the shift
/// ×K; the whole ROW context (scan lines and the jitter hash) comes from the logical row y/K.
pub struct Vhs {
    m: Manifest,
}

impl Vhs {
    pub fn new() -> Vhs {
        Vhs {
            m: Manifest {
                id: "vhs".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("rgb_shift", 0.0, 12.0, 4.0, 1.0, ParamUnit::Px),
                    num("bleed", 0.0, 1.0, 0.4, 0.01, ParamUnit::Norm),
                    num("scan_step", 0.0, 32.0, 4.0, 1.0, ParamUnit::Px),
                    num("jitter", 0.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                ],
                rows: vec![row(Store::U32, false), row(Store::F64, false), row(Store::U32, true), row(Store::F64, true)],
                decl: barrier_decl(),
                ui: UiDecl { name: "VHS", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.vhs", variants: VHS_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

const VHS_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.subtle", sets: &[("rgb_shift", 2.0), ("bleed", 0.2), ("scan_step", 3.0), ("jitter", 0.03)] },
    UiVariant { name_key: "fxreg.var.classic", sets: &[("bleed", 0.5), ("jitter", 0.12)] },
    UiVariant { name_key: "fxreg.var.trash", sets: &[("rgb_shift", 10.0), ("bleed", 0.85), ("scan_step", 6.0), ("jitter", 0.45)] },
];

plugin!(Vhs);

impl BarrierKernel for Vhs {
    /// `[rgb_shift, bleed, scan_step, jitter]`, clamped.
    fn cook(&self, p: &[f64]) -> Cooked {
        [(p[0] as u32).clamp(0, 12) as f64, p[1].clamp(0.0, 1.0), (p[2] as u32).clamp(0, 32) as f64, p[3].clamp(0.0, 1.0), 0.0, 0.0, 0.0, 0.0]
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("vhs.wgsl")]
    }

    /// One pass: `u` = (w, shift ×K, line_off, step, seed, jitter quantum, K), `f0.x` = bleed.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let t = g.burst_index;
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [
                g.w,
                k[0] as u32 * g.res_scale,
                ((t / 3000) % 7) as u32,
                k[2] as u32,
                noise_seed(t),
                (k[3] * 1024.0).round() as i32 as u32,
                g.res_scale,
                0,
            ],
            f: [k[1] as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        });
    }
}
