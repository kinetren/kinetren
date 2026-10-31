//! Stylizing video effects as barrier plugins (plugin-api-barriers): pixelate, edges, halftone,
//! chromatic_aberration, vhs, noise. Pixelate is the pilot (D7): one pass, no SIMD twin, no neighbours read —
//! the pass graph's simplest form. Edges carries the family's AVX2 twins (`simd.rs`).

use std::sync::Arc;

use kt_plugin_api::barrier::{noise_hash, noise_seed, BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
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

mod simd;

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

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let c = k[0] as u32 * cx.res_scale;
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h {
            let sy = (y / c * c + c / 2).min(f.h - 1);
            for x in 0..f.w {
                let sx = (x / c * c + c / 2).min(f.w - 1);
                let d = ((y * f.w + x) * 4) as usize;
                let s = ((sy * f.w + sx) * 4) as usize;
                out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
            }
        }
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
        scratch.put(f.rgba);
        res
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

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let th = k[0] as i32;
        let color = [k[2] as u8, k[3] as u8, k[4] as u8];
        // thickness = the Sobel sampling step; 1 and white colour — the former 3×3 kernel
        // byte-for-byte (including the AVX2 path). R.45 (item 313): the stride and the border ×K;
        // the plain gate (d == 1) switches off by itself at K > 1
        let d = k[1] as i64 * cx.res_scale as i64;
        let plain = d == 1 && color == [255, 255, 255];
        let (w, h) = (f.w as usize, f.h as usize);
        let simd = cfg!(target_arch = "x86_64") && cx.simd && f.rgba.len() >= w * h * 4;
        let luma: Vec<i32> = if simd {
            let mut v = vec![0i32; w * h];
            #[cfg(target_arch = "x86_64")]
            // SAFETY: the host allows SIMD only with AVX2; the buffer lengths — above
            unsafe {
                simd::luma_avx2(&mut v, &f.rgba, w * h)
            };
            v
        } else {
            f.rgba.chunks_exact(4).map(|p| (77 * p[0] as i32 + 150 * p[1] as i32 + 29 * p[2] as i32) >> 8).collect()
        };
        let keep_alpha = f.has_alpha;
        let mut out = scratch.take(f.rgba.len());
        for (o, i) in out.chunks_exact_mut(4).zip(f.rgba.chunks_exact(4)) {
            o[3] = if keep_alpha { i[3] } else { 255 };
        }
        let m = d as usize; // the border is wider with a thick line
        for y in m..h.saturating_sub(m) {
            // the vector part of the row; the scalar loop below finishes the tail
            #[allow(unused_mut)]
            let mut x_from = m;
            #[cfg(target_arch = "x86_64")]
            // the AVX2 kernel writes alpha 255 hard: on a frame WITH ALPHA the scalar path counts
            if plain && simd && w > 2 && !keep_alpha {
                // SAFETY: SIMD is allowed; rows y-1..y+1 exist, out is full
                x_from = unsafe { simd::sobel_row_avx2(&mut out, &luma, w, y, th, w - 1) };
            }
            for x in x_from..w.saturating_sub(m) {
                let l = |dx: i64, dy: i64| luma[(y as i64 + dy * d) as usize * w + (x as i64 + dx * d) as usize];
                let gx = -l(-1, -1) - 2 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2 * l(1, 0) + l(1, 1);
                let gy = -l(-1, -1) - 2 * l(0, -1) - l(1, -1) + l(-1, 1) + 2 * l(0, 1) + l(1, 1);
                let mag = (((gx * gx + gy * gy) as f64).sqrt() as i32).min(255);
                let v = if mag < th { 0 } else { mag as u8 };
                let i = (y * w + x) * 4;
                // colour of the lines: the grey value is multiplied by it (255 — the former)
                for c in 0..3 {
                    out[i + c] = ((v as u32 * color[c] as u32) / 255) as u8;
                }
                if keep_alpha {
                    out[i + 3] = out[i + 3].max(v);
                }
            }
        }
        scratch.put(f.rgba);
        Frame { w: f.w, h: f.h, rgba: out, has_alpha: keep_alpha }
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

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let s = k[0];
        let kk = cx.res_scale as f64;
        let (sa, ca) = k[1].clamp(-90.0, 90.0).to_radians().sin_cos();
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h {
            for x in 0..f.w {
                let d = ((y * f.w + x) * 4) as usize;
                let (lx, ly) = ((x as f64 + 0.5) / kk, (y as f64 + 0.5) / kk);
                // rotation by −angle: the grid stands in the rotated space
                let (ux, uy) = (lx * ca + ly * sa, -lx * sa + ly * ca);
                let (ccx, ccy) = ((ux / s).floor() * s + 0.5 * s, (uy / s).floor() * s + 0.5 * s);
                // cell centre back into logic and into the physics of the frame
                let (gx, gy) = (ccx * ca - ccy * sa, ccx * sa + ccy * ca);
                let smp = cx.raster.sample(&f, gx * kk - 0.5, gy * kk - 0.5);
                let l = (77.0 * smp[0] + 150.0 * smp[1] + 29.0 * smp[2]) / 256.0;
                let rd = 0.5 * s * 1.25 * (1.0 - (l / 255.0).clamp(0.0, 1.0)).sqrt();
                let dist = ((ux - ccx) * (ux - ccx) + (uy - ccy) * (uy - ccy)).sqrt();
                let v = if dist <= rd { 0u8 } else { 255u8 };
                out[d] = v;
                out[d + 1] = v;
                out[d + 2] = v;
                out[d + 3] = f.rgba[d + 3];
            }
        }
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
        scratch.put(f.rgba);
        res
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
            f: [k[0] as f32, a.cos() as f32, a.sin() as f32, g.res_scale as f32, 0.0, 0.0, 0.0, 0.0],
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

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let amount = k[0];
        let (cw, ch) = (f.w as f64 / 2.0, f.h as f64 / 2.0);
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h {
            for x in 0..f.w {
                let d = ((y * f.w + x) * 4) as usize;
                let (px, py) = (x as f64 + 0.5 - cw, y as f64 + 0.5 - ch);
                let r = cx.raster.sample(&f, cw + px * (1.0 - amount) - 0.5, ch + py * (1.0 - amount) - 0.5)[0];
                let b = cx.raster.sample(&f, cw + px * (1.0 + amount) - 0.5, ch + py * (1.0 + amount) - 0.5)[2];
                out[d] = r.round().clamp(0.0, 255.0) as u8;
                out[d + 1] = f.rgba[d + 1];
                out[d + 2] = b.round().clamp(0.0, 255.0) as u8;
                out[d + 3] = f.rgba[d + 3];
            }
        }
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
        scratch.put(f.rgba);
        res
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

/// The grain offset of the common formula (integer division toward zero, as i64).
#[inline(always)]
fn grain_off(gx: u32, gy: u32, ch: u32, seed: u32, k: i64) -> i64 {
    let h = noise_hash(gx ^ noise_hash(gy ^ noise_hash(ch ^ seed)));
    let n = ((h >> 16) & 0x7fff) as i64 - 16384;
    n * k / 32767
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
    /// The offset from the floor of the coordinate of ITS OWN stage; the hash formulas are the
    /// noise kernel's verbatim (both branches).
    #[inline(always)]
    fn tap_at(&self, k: &Cooked, v: &mut [f64; 4], x: f64, y: f64) {
        let (seed, kk, cell) = (k[0] as u32, k[1] as i64, k[2] as u32);
        let (color, special) = (k[3] != 0.0, k[4] != 0.0);
        let (sw, sh) = (k[5] as i64, k[6] as i64);
        let xi = (x.floor() as i64).clamp(0, sw - 1) as u32;
        let yi = (y.floor() as i64).clamp(0, sh - 1) as u32;
        if special {
            let hsh = noise_hash(xi ^ noise_hash(yi ^ seed));
            let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * kk / 32767) as f64;
            for c in v.iter_mut().take(3) {
                *c += off;
            }
        } else {
            let (gx, gy) = (xi / cell, yi / cell);
            if color {
                for (c, val) in v.iter_mut().take(3).enumerate() {
                    *val += grain_off(gx, gy, c as u32 + 1, seed, kk) as f64;
                }
            } else {
                let off = grain_off(gx, gy, 0, seed, kk) as f64;
                for c in v.iter_mut().take(3) {
                    *c += off;
                }
            }
        }
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

    fn frame(&self, kc: &Cooked, mut f: Frame, cx: &BarrierCtx, _scratch: &mut dyn Scratch) -> Frame {
        let (k, size, color) = (kc[0] as i64, kc[1] as u32, kc[2] != 0.0);
        if k == 0 {
            return f;
        }
        let res_scale = cx.res_scale;
        let seed = noise_seed(cx.burst_index);
        let w = f.w;
        // R.45 (item 313): the cell ×K — floor((K·p+i)/(K·size)) == floor(p/size), the grain pattern
        // matches 1× BIT FOR BIT. The formula branch (the special one at size==1 && !color) is
        // chosen by the USER size — the formulas have different grain.
        let cell = size * res_scale;
        if size > 1 || color {
            for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
                let (x, y) = (p as u32 % w / cell, p as u32 / w / cell);
                if color {
                    for (c, v) in px.iter_mut().take(3).enumerate() {
                        *v = (*v as i32 + grain_off(x, y, c as u32 + 1, seed, k) as i32).clamp(0, 255) as u8;
                    }
                } else {
                    let off = grain_off(x, y, 0, seed, k) as i32;
                    for v in px.iter_mut().take(3) {
                        *v = (*v as i32 + off).clamp(0, 255) as u8;
                    }
                }
            }
            return f;
        }
        if res_scale > 1 {
            // special formula on the K× raster: the coordinates are divided by K — the 1× grain
            // pattern is reproduced block by block (the AVX2 path stays with K=1)
            for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
                let (x, y) = (p as u32 % w / res_scale, p as u32 / w / res_scale);
                let h = noise_hash(x ^ noise_hash(y ^ seed));
                let n = ((h >> 16) & 0x7fff) as i64 - 16384;
                let off = (n * k / 32767) as i32;
                for c in px.iter_mut().take(3) {
                    *c = (*c as i32 + off).clamp(0, 255) as u8;
                }
            }
            return f;
        }
        #[cfg(target_arch = "x86_64")]
        if cx.simd && w >= 8 && f.rgba.len() >= (w as usize) * (f.h as usize) * 4 {
            // SAFETY: the host allows SIMD only with AVX2, the buffer length — above
            unsafe { simd::noise_avx2(&mut f.rgba, w, f.h, seed, k) };
            // the column tail (w % 8) — with the same formula as the scalar below
            let vec_w = w & !7;
            if vec_w < w {
                for y in 0..f.h {
                    for x in vec_w..w {
                        let i = ((y * w + x) * 4) as usize;
                        let h = noise_hash(x ^ noise_hash(y ^ seed));
                        let n = ((h >> 16) & 0x7fff) as i64 - 16384;
                        let off = (n * k / 32767) as i32;
                        for c in 0..3 {
                            f.rgba[i + c] = (f.rgba[i + c] as i32 + off).clamp(0, 255) as u8;
                        }
                    }
                }
            }
            return f;
        }
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w, p as u32 / w);
            let h = noise_hash(x ^ noise_hash(y ^ seed));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * k / 32767) as i32;
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + off).clamp(0, 255) as u8;
            }
        }
        f
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

    fn frame(&self, k: &Cooked, mut f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let burst_index = cx.burst_index;
        let rs = cx.res_scale as usize;
        let shift = k[0] as i64 * cx.res_scale as i64;
        let bleed = k[1];
        let line_off = ((burst_index / 3000) % 7) as usize;
        let (w, h) = (f.w as usize, f.h as usize);
        // scan_step = 4 and jitter = 0 — the former behaviour byte-for-byte (including the AVX2
        // path); everything else is computed by the scalar path
        let step = k[2] as usize;
        let jitter = k[3];
        let plain = step == 4 && jitter == 0.0;
        let jq = (jitter * 1024.0).round() as i64;
        let seed = noise_seed(burst_index);
        // only the pixels of the CURRENT row are read → a row scratch instead of a full clone
        let mut row = scratch.take(w * 4);
        for y in 0..h {
            let scan = if step > 0 && (y / rs + line_off).is_multiple_of(step) { 1i64 } else { 0 };
            // the row jitter: a shift of the whole row by a width fraction, INTEGER (GPU round()
            // rounds halves to even, t181); the parameter quantization is the same for both paths
            let jx = if jq == 0 {
                0i64
            } else {
                let hsh = noise_hash((y / rs) as u32 ^ noise_hash(seed));
                let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
                (n * jq / 32767) * w as i64 / 4096
            };
            let r0 = y * w * 4;
            row.copy_from_slice(&f.rgba[r0..r0 + w * 4]);
            // the vector interior of the row; the edges and the tail — by a scalar loop
            #[allow(unused_mut)]
            let mut x_from = 0usize;
            #[cfg(target_arch = "x86_64")]
            if plain && cx.simd {
                let d = (shift + scan) as usize;
                // SAFETY: the host allows SIMD only with AVX2; row of length w*4, the slice too
                x_from = unsafe { simd::vhs_row_avx2(&mut f.rgba[r0..r0 + w * 4], &row, w, d, bleed, scan == 1, 0) };
            }
            // the left edge (the vector skipped it, starting at x = d)
            if x_from > 0 {
                for x in 0..(shift + scan) as usize {
                    let i = r0 + x * 4;
                    let xr = (x as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                    let xb = (x as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                    let r = row[xr * 4] as f64;
                    let b = row[xb * 4 + 2] as f64;
                    f.rgba[i] = (row[x * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                    f.rgba[i + 2] = (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                    if scan == 1 {
                        f.rgba[i + 1] = row[x * 4 + 1].saturating_sub(18);
                    }
                }
            }
            for x in x_from..w {
                let i = r0 + x * 4;
                // the row base drifts by jx (0 — the former formula verbatim)
                let xj = (x as i64 + jx).clamp(0, w as i64 - 1) as usize;
                let xr = (xj as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                let xb = (xj as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                let r = row[xr * 4] as f64;
                let b = row[xb * 4 + 2] as f64;
                f.rgba[i] = (row[xj * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                f.rgba[i + 2] = (row[xj * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                if scan == 1 {
                    f.rgba[i + 1] = row[xj * 4 + 1].saturating_sub(18);
                } else if jx != 0 {
                    // with jitter the WHOLE row moves, including the green (an in-place artefact
                    // otherwise, because of it CPU and GPU diverged)
                    f.rgba[i + 1] = row[xj * 4 + 1];
                }
            }
        }
        scratch.put(row);
        f
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

/// The AVX2 form of noise's integer kernel, for t198 (kt-fx `tests/simd_parity.rs`; the scalar
/// reference copy stays in `kt_fx::simd_testing`).
pub mod simd_testing {
    /// The AVX2 kernel of noise (in place over the whole frame, the column tail is not touched —
    /// the test finishes it with the scalar path). Panics without AVX2.
    #[cfg(target_arch = "x86_64")]
    pub fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked; the buffer length is the test's
        unsafe { crate::simd::noise_avx2(rgba, w, h, seed, k) };
    }
}
