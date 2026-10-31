//! Stylizing video effects as barrier plugins (plugin-api-barriers): pixelate, edges, halftone,
//! chromatic_aberration. Pixelate is the pilot (D7): one pass, no SIMD twin, no neighbours read —
//! the pass graph's simplest form. Edges carries the family's AVX2 twins (`simd.rs`).

use std::sync::Arc;

use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, UiDecl, UiVariant,
    VideoClass,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(Pixelate::new()), Arc::new(Edges::new()), Arc::new(Halftone::new()), Arc::new(ChromaticAberration::new())]
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
