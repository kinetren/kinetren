//! Blurring and glowing video effects as barrier plugins (plugin-api-barriers, stage 3): blur, glow,
//! sharpen, radial_blur, drop_shadow. The mip of blur, glow, sharpen and drop_shadow is the host's
//! resample (`RasterHost::resample` on the CPU, the `Resample` node on the GPU) — one text with the
//! core's own resample and its AVX2 twin; each kernel below is the former kt-fx kernel verbatim.

use std::sync::Arc;

use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, UiDecl, UiVariant,
    VideoClass,
};

mod simd;

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(Blur::new()),
        Arc::new(Glow::new()),
        Arc::new(Sharpen::new()),
        Arc::new(RadialBlur::new()),
        Arc::new(DropShadow::new()),
    ]
}

fn decl() -> Decl {
    Decl { video_class: Some(VideoClass::Barrier), gpu: Gpu::Barrier, ..Decl::default() }
}

fn num(name: &'static str, min: f64, max: f64, default: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec::num(name, min, max, default, step, unit, ParamKind::Smooth)
}

fn row(store: Store, omit_default: bool) -> RowDecl {
    RowDecl { store, omit_default }
}

/// The host's `Pass` with up to three inputs.
fn pass(shader: u8, inputs: [Slot; 3], u: [u32; 8], f: [f32; 8]) -> Node {
    Node::Pass { shader, inputs, u, f }
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

// ---- blur -----------------------------------------------------------------------------------

/// Box-mip blur: down by the step and back up (bilinear both ways). Over the PREMULTIPLIED image
/// when the frame has alpha (R.53 item 418), otherwise the colour of transparent pixels leaks into
/// the visible ones; on an opaque frame both conversions are identities.
pub struct Blur {
    m: Manifest,
}

impl Blur {
    pub fn new() -> Blur {
        Blur {
            m: Manifest {
                id: "blur".into(),
                media: Media::VideoFx,
                passport: vec![num("radius", 1.0, 48.0, 8.0, 1.0, ParamUnit::Px), num("radius_y", 0.0, 48.0, 0.0, 1.0, ParamUnit::Px)],
                rows: vec![row(Store::U32, false), row(Store::U32, true)],
                decl: decl(),
                ui: UiDecl { name: "Blur", category_key: "fxreg.cat.blur", keywords_key: "fxreg.kw.blur", variants: BLUR_VARIANTS, ..UiDecl::default() },
            },
        }
    }

    /// The mip's steps at K (R.45 item 313: the K× execution scales the STEP — floor(K·w/(K·step))
    /// == floor(w/step), the mip is the same, the look is invariant exactly).
    fn steps(k: &Cooked, rs: u32) -> (u32, u32) {
        let (radius, radius_y) = (k[0] as u32, k[1] as u32);
        let step = ((radius / 3).max(1) + 1) * rs;
        let step_y = if radius_y == 0 { step } else { ((radius_y.clamp(1, 48) / 3).max(1) + 1) * rs };
        (step, step_y)
    }
}

const BLUR_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.soft", sets: &[("radius", 3.0)] },
    UiVariant { name_key: "fxreg.var.heavy", sets: &[("radius", 24.0)] },
    // blur along one axis: radius_y = 1 (not 0 — zero means "as X")
    UiVariant { name_key: "fxreg.var.horizontal", sets: &[("radius", 28.0), ("radius_y", 1.0)] },
];

plugin!(Blur);

impl BarrierKernel for Blur {
    fn cook(&self, p: &[f64]) -> Cooked {
        [(p[0] as u32).clamp(1, 48) as f64, p[1], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn frame(&self, k: &Cooked, mut f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let (step, step_y) = Blur::steps(k, cx.res_scale);
        let (dw, dh) = ((f.w / step).max(1), (f.h / step_y).max(1));
        let pm = f.has_alpha;
        if pm {
            cx.raster.premultiply(&mut f.rgba);
        }
        let small = cx.raster.resample(&f, dw, dh, scratch);
        let mut out = cx.raster.resample(&small, f.w, f.h, scratch);
        if pm {
            cx.raster.unpremultiply(&mut out.rgba);
        }
        scratch.put(small.rgba);
        scratch.put(f.rgba);
        Frame { w: f.w, h: f.h, rgba: out.rgba, has_alpha: f.has_alpha }
    }

    /// The mip down and up, wrapped in premultiplication when the frame here has alpha (a mirror
    /// of the gate above: on an opaque frame the passes are not set up at all).
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let (step, step_y) = Blur::steps(k, g.res_scale);
        let (dw, dh) = ((g.w / step).max(1), (g.h / step_y).max(1));
        let pm = g.has_alpha();
        if pm {
            out.push(Node::Premul);
        }
        out.push(Node::Resample { w: dw, h: dh });
        out.push(Node::Resample { w: g.w, h: g.h });
        if pm {
            out.push(Node::Unpremul);
        }
    }
}

// ---- glow -----------------------------------------------------------------------------------

/// Bloom: the frame plus its blurred copy ×strength, a soft threshold by the blurred luma and a
/// tint. Light does not come from transparent places (R.53 item 418): the mip is taken from a
/// PREMULTIPLIED copy and added as is; the base of the addition stays straight.
pub struct Glow {
    m: Manifest,
}

impl Glow {
    pub fn new() -> Glow {
        let rgb = |n: &'static str| num(n, 0.0, 255.0, 255.0, 1.0, ParamUnit::Rgb8);
        Glow {
            m: Manifest {
                id: "glow".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("strength", 0.0, 2.0, 0.65, 0.01, ParamUnit::Ratio),
                    num("radius", 1.0, 32.0, 4.0, 1.0, ParamUnit::Px),
                    num("threshold", 0.0, 255.0, 0.0, 1.0, ParamUnit::None),
                    rgb("tint_r"),
                    rgb("tint_g"),
                    rgb("tint_b"),
                ],
                rows: vec![
                    row(Store::F64, false),
                    row(Store::U32, true),
                    row(Store::U32, true),
                    row(Store::U8, true),
                    row(Store::U8, true),
                    row(Store::U8, true),
                ],
                decl: decl(),
                ui: UiDecl { name: "Glow", category_key: "fxreg.cat.glow", keywords_key: "fxreg.kw.glow", variants: GLOW_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

const GLOW_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.soft", sets: &[("strength", 0.4), ("radius", 8.0), ("threshold", 128.0)] },
    UiVariant { name_key: "fxreg.var.dream", sets: &[("strength", 1.1), ("radius", 16.0), ("threshold", 96.0)] },
    UiVariant {
        name_key: "fxreg.var.golden",
        sets: &[("strength", 0.9), ("radius", 12.0), ("threshold", 110.0), ("tint_g", 200.0), ("tint_b", 120.0)],
    },
];

plugin!(Glow);

impl BarrierKernel for Glow {
    /// `[s, radius, threshold, tint_r, tint_g, tint_b]`, s = strength·0.35 (0 — no glow at all).
    fn cook(&self, p: &[f64]) -> Cooked {
        let s = p[0].clamp(0.0, 2.0) * 0.35;
        [s, (p[1] as u32).clamp(1, 32) as f64, (p[2] as u32).min(255) as f64, p[3], p[4], p[5], 0.0, 0.0]
    }

    fn frame(&self, k: &Cooked, mut f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let s = k[0];
        if s <= 0.0 {
            return f;
        }
        // radius 4, threshold 0 and white tint — the former path byte-for-byte
        // R.45 (item 313): the mip divisor ×K — the mip is the same, the look is invariant
        let r = k[1] as u32 * cx.res_scale;
        let th = k[2] as i32;
        let tint = [k[3] as u8, k[4] as u8, k[5] as u8];
        let plain = th == 0 && tint == [255, 255, 255];
        let (dw, dh) = ((f.w / r).max(1), (f.h / r).max(1));
        let pmbuf = f.has_alpha.then(|| {
            let mut b = scratch.take(f.rgba.len());
            b.copy_from_slice(&f.rgba);
            cx.raster.premultiply(&mut b);
            Frame { w: f.w, h: f.h, rgba: b, has_alpha: true }
        });
        let small = cx.raster.resample(pmbuf.as_ref().unwrap_or(&f), dw, dh, scratch);
        if let Some(fr) = pmbuf {
            scratch.put(fr.rgba);
        }
        let up = cx.raster.resample(&small, f.w, f.h, scratch);
        // in-place addition: only the pixel itself + up is read (item 2 of round 7)
        let pixels = f.rgba.len().min(up.rgba.len()) / 4;
        let mut done = 0usize;
        #[cfg(target_arch = "x86_64")]
        if plain && cx.simd {
            // SAFETY: the host allows SIMD only when the CPU has AVX2; pixels do not go past
            // either buffer
            unsafe { simd::glow_add_avx2(&mut f.rgba, &up.rgba, s, pixels) };
            done = pixels;
        }
        for (o, u) in f.rgba.chunks_exact_mut(4).zip(up.rgba.chunks_exact(4)).skip(done) {
            // threshold by luma of the BLURRED frame: a soft knee of width th
            let w = if th == 0 {
                1.0
            } else {
                let luma = (77 * u[0] as i32 + 150 * u[1] as i32 + 29 * u[2] as i32) >> 8;
                (((luma - th) as f64) / th as f64).clamp(0.0, 1.0)
            };
            for c in 0..3 {
                let k = s * w * tint[c] as f64 / 255.0;
                o[c] = (o[c] as f64 + u[c] as f64 * k).min(255.0) as u8;
            }
        }
        scratch.put(small.rgba);
        scratch.put(up.rgba);
        f
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("glow_add.wgsl")]
    }

    /// [premul copy], the mip down and up, then the addition over the straight source (T0) with
    /// the upscaled mip (T1): `u0.x` = threshold, `f0` = (s, tint).
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        if k[0] <= 0.0 {
            return;
        }
        let r = k[1] as u32 * g.res_scale;
        if g.has_alpha() {
            out.push(Node::Premul);
        }
        out.push(Node::Resample { w: (g.w / r).max(1), h: (g.h / r).max(1) });
        out.push(Node::Resample { w: g.w, h: g.h });
        out.push(pass(
            0,
            [Slot::Src, Slot::Prev, Slot::None],
            [k[2] as u32, 0, 0, 0, 0, 0, 0, 0],
            [k[0] as f32, k[3] as f32, k[4] as f32, k[5] as f32, 0.0, 0.0, 0.0, 0.0],
        ));
    }
}

// ---- sharpen --------------------------------------------------------------------------------

/// Unsharp masking by mip (R.47, F-FX-001): source + (source − blurred base)·amount.
pub struct Sharpen {
    m: Manifest,
}

impl Sharpen {
    pub fn new() -> Sharpen {
        Sharpen {
            m: Manifest {
                id: "sharpen".into(),
                media: Media::VideoFx,
                passport: vec![num("amount", 0.0, 4.0, 1.0, 0.01, ParamUnit::Ratio), num("radius", 1.0, 16.0, 2.0, 1.0, ParamUnit::Px)],
                rows: vec![row(Store::F32, false), row(Store::U32, false)],
                decl: decl(),
                ui: UiDecl { name: "Sharpen", category_key: "fxreg.cat.blur", keywords_key: "fxreg.kw.sharpen", variants: SHARPEN_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

const SHARPEN_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.soft", sets: &[("amount", 0.5), ("radius", 1.0)] },
    UiVariant { name_key: "fxreg.var.strong", sets: &[("amount", 2.0)] },
    UiVariant { name_key: "fxreg.var.heavy", sets: &[("amount", 3.2), ("radius", 4.0)] },
];

plugin!(Sharpen);

impl BarrierKernel for Sharpen {
    /// `[amount, radius]`: the amount clamped in f64 (the CPU's), the radius to its 2..16.
    fn cook(&self, p: &[f64]) -> Cooked {
        [p[0].clamp(0.0, 4.0), (p[1] as u32).clamp(2, 16) as f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let amount = k[0];
        let step = k[1] as u32 * cx.res_scale;
        let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
        let small = cx.raster.resample(&f, dw, dh, scratch);
        let base = cx.raster.resample(&small, f.w, f.h, scratch);
        scratch.put(small.rgba);
        let mut out = f;
        for (px, bp) in out.rgba.chunks_exact_mut(4).zip(base.rgba.chunks_exact(4)) {
            for c in 0..3 {
                let s = px[c] as f64;
                px[c] = (s + (s - bp[c] as f64) * amount).round().clamp(0.0, 255.0) as u8;
            }
        }
        scratch.put(base.rgba);
        out
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("sharpen_add.wgsl")]
    }

    /// The mip down and up, then the addition: T0 = the source, T1 = the base, `f0.x` = amount.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let step = k[1] as u32 * g.res_scale;
        out.push(Node::Resample { w: (g.w / step).max(1), h: (g.h / step).max(1) });
        out.push(Node::Resample { w: g.w, h: g.h });
        out.push(pass(0, [Slot::Src, Slot::Prev, Slot::None], [0; 8], [k[0] as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
    }
}

// ---- radial blur ----------------------------------------------------------------------------

/// Radial blur (R.47, F-FX-003): a zoom streak or a rotation around the centre, EXACTLY 16 taps on
/// both backends; positions are frame fractions, so the K× execution scales itself.
pub struct RadialBlur {
    m: Manifest,
}

impl RadialBlur {
    pub fn new() -> RadialBlur {
        RadialBlur {
            m: Manifest {
                id: "radial_blur".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("amount", 0.0, 1.0, 0.25, 0.01, ParamUnit::Norm),
                    num("center_x", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                    num("center_y", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                    ParamSpec::flag_row("spin", false),
                ],
                rows: vec![row(Store::F32, false), row(Store::F32, true), row(Store::F32, true), row(Store::Bool, true)],
                decl: decl(),
                ui: UiDecl {
                    name: "Radial Blur",
                    category_key: "fxreg.cat.blur",
                    keywords_key: "fxreg.kw.radial_blur",
                    variants: RADIAL_VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

const RADIAL_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.zoom", sets: &[("amount", 0.5)] },
    // rotation instead of zoom
    UiVariant { name_key: "fxreg.var.spin", sets: &[("amount", 0.4), ("spin", 1.0)] },
    UiVariant { name_key: "fxreg.var.heavy", sets: &[("amount", 0.85)] },
];

plugin!(RadialBlur);

impl BarrierKernel for RadialBlur {
    /// `[amount, center_x, center_y, spin]` as the rows hold them (f32 values in f64).
    fn cook(&self, p: &[f64]) -> Cooked {
        [p[0], p[1], p[2], p[3], 0.0, 0.0, 0.0, 0.0]
    }

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        const N: usize = 16;
        let (amount, spin) = (k[0].clamp(0.0, 1.0), k[3] != 0.0);
        let (ccx, ccy) = ((0.5 + k[1].clamp(-1.0, 1.0) * 0.5) * f.w as f64, (0.5 + k[2].clamp(-1.0, 1.0) * 0.5) * f.h as f64);
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h {
            for x in 0..f.w {
                let d = ((y * f.w + x) * 4) as usize;
                let (px, py) = (x as f64 + 0.5 - ccx, y as f64 + 0.5 - ccy);
                let mut acc = [0f64; 4];
                for i in 0..N {
                    let t = i as f64 / (N - 1) as f64;
                    let (sx, sy) = if spin {
                        let a = 0.6 * amount * (t - 0.5);
                        let (sa, ca) = a.sin_cos();
                        (px * ca - py * sa, px * sa + py * ca)
                    } else {
                        let k = 1.0 - 0.5 * amount * t;
                        (px * k, py * k)
                    };
                    let s = cx.raster.sample(&f, ccx + sx - 0.5, ccy + sy - 0.5);
                    for (a, v) in acc.iter_mut().zip(s) {
                        *a += v;
                    }
                }
                for (c, a) in acc.iter().enumerate() {
                    out[d + c] = (a / N as f64).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
        scratch.put(f.rgba);
        res
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("radial_blur.wgsl")]
    }

    /// One pass: `u0` = (w, h, spin), `f0` = (amount, cx, cy) — the centre in f32 arithmetic, as the
    /// GPU arm computed it before the move.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        let (cxf, cyf) = (k[1] as f32, k[2] as f32);
        out.push(pass(
            0,
            [Slot::Prev, Slot::None, Slot::None],
            [g.w, g.h, (k[3] != 0.0) as u32, 0, 0, 0, 0, 0],
            [(k[0] as f32).clamp(0.0, 1.0), 0.5 + cxf.clamp(-1.0, 1.0) * 0.5, 0.5 + cyf.clamp(-1.0, 1.0) * 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
        ));
    }
}

// ---- drop shadow ----------------------------------------------------------------------------

/// Shadow under the layer (R.47): the source alpha blurred (mip radius·K) and shifted by offset·K;
/// the source OVER the shadow (straight alpha, f64, rounding once). An input without alpha is a
/// passthrough: the formula on an opaque input degenerates into identity, the early exit only saves
/// work, and the GPU graph is empty there.
pub struct DropShadow {
    m: Manifest,
}

impl DropShadow {
    pub fn new() -> DropShadow {
        let rgb = |n: &'static str| num(n, 0.0, 255.0, 0.0, 1.0, ParamUnit::Rgb8);
        DropShadow {
            m: Manifest {
                id: "drop_shadow".into(),
                media: Media::VideoFx,
                passport: vec![
                    num("offset_x", -64.0, 64.0, 8.0, 1.0, ParamUnit::Px),
                    num("offset_y", -64.0, 64.0, 8.0, 1.0, ParamUnit::Px),
                    num("radius", 1.0, 32.0, 4.0, 1.0, ParamUnit::Px),
                    num("opacity", 0.0, 1.0, 0.6, 0.01, ParamUnit::Norm),
                    rgb("tint_r"),
                    rgb("tint_g"),
                    rgb("tint_b"),
                ],
                rows: vec![
                    row(Store::I32, false),
                    row(Store::I32, false),
                    row(Store::U32, false),
                    row(Store::F32, false),
                    row(Store::U8, true),
                    row(Store::U8, true),
                    row(Store::U8, true),
                ],
                decl: decl(),
                ui: UiDecl {
                    name: "Drop Shadow",
                    category_key: "fxreg.cat.key",
                    keywords_key: "fxreg.kw.drop_shadow",
                    variants: SHADOW_VARIANTS,
                    alpha_sample: true,
                    ..UiDecl::default()
                },
            },
        }
    }
}

const SHADOW_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.soft", sets: &[("offset_x", 6.0), ("offset_y", 6.0), ("radius", 12.0), ("opacity", 0.45)] },
    UiVariant { name_key: "fxreg.var.hard", sets: &[("offset_x", 4.0), ("offset_y", 4.0), ("radius", 1.0), ("opacity", 0.8)] },
    UiVariant { name_key: "fxreg.var.far", sets: &[("offset_x", 24.0), ("offset_y", 24.0), ("radius", 16.0), ("opacity", 0.5)] },
];

plugin!(DropShadow);

impl BarrierKernel for DropShadow {
    /// `[offset_x, offset_y, radius, opacity, tint_r, tint_g, tint_b]`, clamped.
    fn cook(&self, p: &[f64]) -> Cooked {
        [
            (p[0] as i32).clamp(-64, 64) as f64,
            (p[1] as i32).clamp(-64, 64) as f64,
            (p[2] as u32).clamp(1, 32) as f64,
            p[3].clamp(0.0, 1.0),
            p[4],
            p[5],
            p[6],
            0.0,
        ]
    }

    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        if !f.has_alpha {
            return f;
        }
        let rs = cx.res_scale;
        let step = k[2] as u32 * rs;
        let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
        let small = cx.raster.resample(&f, dw, dh, scratch);
        let blurred = cx.raster.resample(&small, f.w, f.h, scratch);
        scratch.put(small.rgba);
        let (ox, oy) = (k[0] as i32 * rs as i32, k[1] as i32 * rs as i32);
        let (opacity, tint) = (k[3], [k[4] as u8, k[5] as u8, k[6] as u8]);
        let mut out = scratch.take(f.rgba.len());
        for y in 0..f.h as i64 {
            for x in 0..f.w as i64 {
                let d = ((y * f.w as i64 + x) * 4) as usize;
                let (bx, by) = (x - ox as i64, y - oy as i64);
                let ba = if bx >= 0 && by >= 0 && bx < f.w as i64 && by < f.h as i64 {
                    blurred.rgba[((by * f.w as i64 + bx) * 4 + 3) as usize] as f64 / 255.0
                } else {
                    0.0
                };
                let ash = ba * opacity;
                let asrc = f.rgba[d + 3] as f64 / 255.0;
                let oa = asrc + ash * (1.0 - asrc);
                if oa <= 0.0 {
                    out[d..d + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else {
                    for c in 0..3 {
                        let v = (f.rgba[d + c] as f64 * asrc + tint[c] as f64 * ash * (1.0 - asrc)) / oa;
                        out[d + c] = v.round().clamp(0.0, 255.0) as u8;
                    }
                    out[d + 3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        scratch.put(blurred.rgba);
        let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: true };
        scratch.put(f.rgba);
        res
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("shadow_under.wgsl")]
    }

    /// Opaque input (the fold KNOWS it): no passes — the CPU's passthrough. With unknown alpha the
    /// graph is set up: the formula on an opaque input is identical in bytes.
    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        if g.alpha == Some(false) {
            return;
        }
        let step = k[2] as u32 * g.res_scale;
        out.push(Node::Resample { w: (g.w / step).max(1), h: (g.h / step).max(1) });
        out.push(Node::Resample { w: g.w, h: g.h });
        let (ox, oy) = (k[0] as i32 * g.res_scale as i32, k[1] as i32 * g.res_scale as i32);
        out.push(pass(
            0,
            [Slot::Src, Slot::Prev, Slot::None],
            [g.w, g.h, ox as u32, oy as u32, 0, 0, 0, 0],
            [(k[3] as f32).clamp(0.0, 1.0), k[4] as f32, k[5] as f32, k[6] as f32, 0.0, 0.0, 0.0, 0.0],
        ));
    }
}

/// The scalar and AVX2 forms of glow's additive finish, for t198 (kt-fx `tests/simd_parity.rs`).
pub mod simd_testing {
    /// The scalar loop of glow's finish at threshold 0 and a white tint.
    pub fn glow_add_scalar(dst: &mut [u8], up: &[u8], s: f64) {
        for (o, u) in dst.chunks_exact_mut(4).zip(up.chunks_exact(4)) {
            for c in 0..3 {
                o[c] = (o[c] as f64 + u[c] as f64 * s).min(255.0) as u8;
            }
        }
    }

    /// The AVX2 twin; panics without AVX2 (the calling test checks first).
    #[cfg(target_arch = "x86_64")]
    pub fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked
        unsafe { crate::simd::glow_add_avx2(dst, up, s, pixels) };
    }
}
