//! Colour video effects as plugins (plugin-api-stage1): color_grade, duotone, posterize, vignette.
//! Posterize is the pilot (D9): the smallest kernel that crosses all four executor positions — the
//! CPU barrier, the CPU tap of the fused pass, its own GPU pass and the GPU tap arm.

use std::sync::Arc;

use kt_plugin_api::barrier::{AlphaRel, BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{UiVariant, 
    Cooked, Decl, Gpu, Manifest, TapCtx, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store,
    UiDecl, VideoClass, VideoKernel,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(ColorGrade::new()),
        Arc::new(Duotone::new()),
        Arc::new(Posterize::new()),
        Arc::new(Vignette::new()),
        Arc::new(Greenscreen::new()),
    ]
}

/// Posterize by channels (R.47): point-wise (inside a geometry series the supersampling softens
/// the steps — accepted, THRESHOLD).
pub struct Posterize {
    m: Manifest,
}

impl Posterize {
    pub fn new() -> Posterize {
        Posterize {
            m: Manifest {
                id: "posterize".into(),
                media: Media::VideoFx,
                // the row the variant declared: levels u32 2..32, default 6
                passport: vec![ParamSpec::num("levels", 2.0, 32.0, 6.0, 1.0, ParamUnit::Count, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::U32, omit_default: false }],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("posterize.wgsl")),
                    ..Decl::default()
                },
                ui: UiDecl { name: "Posterize", category_key: "fxreg.cat.color", keywords_key: "fxreg.kw.posterize", variants: POSTERIZE_VARIANTS, ..UiDecl::default() },
            },
        }
    }

    fn levels(p: &[f64]) -> u32 {
        (p.first().copied().unwrap_or(6.0).round() as u32).clamp(2, 32)
    }
}

/// `n` levels per channel, RGB only.
#[inline(always)]
fn quantize(n: f64, v: &mut [f64; 4]) {
    for c in v.iter_mut().take(3) {
        let i = (*c * n / 256.0).floor().clamp(0.0, n - 1.0);
        *c = i * 255.0 / (n - 1.0);
    }
}

impl Default for Posterize {
    fn default() -> Posterize {
        Posterize::new()
    }
}

impl Plugin for Posterize {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
}

impl VideoKernel for Posterize {
    /// `levels` clamped and cast once per frame (the variant held a `u32`).
    fn cook(&self, p: &[f64]) -> Cooked {
        [Posterize::levels(p) as f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }
    /// Quantization PER CHANNEL (the standard — AE Posterize); alpha untouched.
    fn tap(&self, k: &Cooked, v: &mut [f64; 4]) {
        quantize(k[0], v);
    }
    /// The barrier loop with `n` taken through its `u32` range once per frame: the same value,
    /// and the compiler knows `n - 1 >= 1` (the variant's `u32` field gave it that; without it the
    /// loop was 6 % slower than the variant's, plugin-api-stage1 2.6). Inside the fused pass the
    /// cooked value is read as is — there the per-sample cast cost 8 %.
    fn frame(&self, k: &Cooked, rgba: &mut [u8], _w: u32) {
        let n = (k[0] as u32).clamp(2, 32) as f64;
        kt_plugin_api::frame_with(rgba, |v| quantize(n, v));
    }
    fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
        [Posterize::levels(p) as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }
}

// ---------------------------------------------------------------------------
// color_grade, duotone (stage 4: moved from kt-fx `geom.rs` / kt-gpu, verbatim)
// ---------------------------------------------------------------------------

/// Color grading on the subtap: luma (Rec.601 8-bit), saturation, hue, then
/// contrast around 128 and brightness (the order is fixed — it is what keeps it from clipping
/// hues). Hue — a rotation around the gray axis (Rodrigues, k = (1,1,1)/√3).
/// The values are an f64 continuum; the final clamp+round — once, at the output.
#[inline(always)]
fn grade(v: &mut [f64; 4], br: f64, con: f64, sat: f64, hc: f64, hs: f64) {
    let l = (77.0 * v[0] + 150.0 * v[1] + 29.0 * v[2]) / 256.0;
    for c in v.iter_mut().take(3) {
        *c = l + (*c - l) * sat;
    }
    if hc != 1.0 || hs != 0.0 {
        let k = 1.0 / 3f64.sqrt();
        let (x, y, z) = (v[0], v[1], v[2]);
        let dot = k * (x + y + z);
        let (cx, cy, cz) = (k * (y - z), k * (z - x), k * (x - y));
        v[0] = x * hc + cx * hs + k * dot * (1.0 - hc);
        v[1] = y * hc + cy * hs + k * dot * (1.0 - hc);
        v[2] = z * hc + cz * hs + k * dot * (1.0 - hc);
    }
    for c in v.iter_mut().take(3) {
        *c = (*c - 128.0) * (1.0 + con) + 128.0 + br * 255.0;
    }
}

/// Duotone on the subtap: lerp of two colors by luma; alpha untouched.
#[inline(always)]
fn duo(v: &mut [f64; 4], shadow: &[f64], high: &[f64], amount: f64) {
    let l = ((77.0 * v[0] + 150.0 * v[1] + 29.0 * v[2]) / 256.0 / 255.0).clamp(0.0, 1.0);
    for (c, (sh, hi)) in v.iter_mut().take(3).zip(shadow.iter().zip(high.iter())) {
        let out = sh + (hi - sh) * l;
        *c += (out - *c) * amount;
    }
}

/// Color grading (R.47): brightness, contrast, saturation, hue — point-wise.
pub struct ColorGrade {
    m: Manifest,
}

impl ColorGrade {
    pub fn new() -> ColorGrade {
        let p = |n, lo, hi, d, st, u| ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth);
        ColorGrade {
            m: Manifest {
                id: "color_grade".into(),
                media: Media::VideoFx,
                passport: vec![
                    p("brightness", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                    p("contrast", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm),
                    p("saturation", 0.0, 2.0, 1.0, 0.01, ParamUnit::Ratio),
                    p("hue", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                ],
                // every field was `skip_serializing_if` at its default
                rows: vec![f32_row(true), f32_row(true), f32_row(true), f32_row(true)],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("color_grade.wgsl")),
                    ..Decl::default()
                },
                ui: UiDecl { name: "Color Grade", category_key: "fxreg.cat.color", keywords_key: "fxreg.kw.color_grade", variants: COLOR_GRADE_VARIANTS, ..UiDecl::default() },
            },
        }
    }

    /// (brightness, contrast, saturation, hue_cos, hue_sin) as the planner gave them.
    fn numbers(p: &[f64]) -> [f64; 5] {
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
        let h = g(3).to_radians();
        [g(0), g(1), p.get(2).copied().unwrap_or(1.0), h.cos(), h.sin()]
    }
}

impl Default for ColorGrade {
    fn default() -> ColorGrade {
        ColorGrade::new()
    }
}

impl Plugin for ColorGrade {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
}

impl VideoKernel for ColorGrade {
    /// The hue's cos/sin once per plan (the variant's planner did the same).
    fn cook(&self, p: &[f64]) -> Cooked {
        let n = ColorGrade::numbers(p);
        [n[0], n[1], n[2], n[3], n[4], 0.0, 0.0, 0.0]
    }
    fn tap(&self, k: &Cooked, v: &mut [f64; 4]) {
        grade(v, k[0], k[1], k[2], k[3], k[4]);
    }
    fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
        let n = ColorGrade::numbers(p);
        [n[0] as f32, n[1] as f32, n[2] as f32, n[3] as f32, n[4] as f32, 0.0, 0.0]
    }
}

/// Duotone (R.47): luma mapped between two colors.
pub struct Duotone {
    m: Manifest,
}

impl Duotone {
    pub fn new() -> Duotone {
        let c = |n, d| ParamSpec::num(n, 0.0, 255.0, d, 1.0, ParamUnit::Rgb8, ParamKind::Smooth);
        let u8_row = RowDecl { store: Store::U8, omit_default: false };
        Duotone {
            m: Manifest {
                id: "duotone".into(),
                media: Media::VideoFx,
                passport: vec![
                    c("shadow_r", 24.0),
                    c("shadow_g", 18.0),
                    c("shadow_b", 64.0),
                    c("high_r", 255.0),
                    c("high_g", 236.0),
                    c("high_b", 190.0),
                    ParamSpec::num("amount", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm, ParamKind::Smooth),
                ],
                rows: vec![u8_row, u8_row, u8_row, u8_row, u8_row, u8_row, f32_row(false)],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("duotone.wgsl")),
                    ..Decl::default()
                },
                ui: UiDecl { name: "Duotone", category_key: "fxreg.cat.color", keywords_key: "fxreg.kw.duotone", variants: DUOTONE_VARIANTS, ..UiDecl::default() },
            },
        }
    }
}

impl Default for Duotone {
    fn default() -> Duotone {
        Duotone::new()
    }
}

impl Plugin for Duotone {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
}

impl VideoKernel for Duotone {
    fn cook(&self, p: &[f64]) -> Cooked {
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
        [g(0), g(1), g(2), g(3), g(4), g(5), g(6).clamp(0.0, 1.0), 0.0]
    }
    fn tap(&self, k: &Cooked, v: &mut [f64; 4]) {
        duo(v, &k[0..3], &k[3..6], k[6]);
    }
    fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0) as f32;
        [g(0), g(1), g(2), g(3), g(4), g(5), g(6).clamp(0.0, 1.0)]
    }
}

/// An `f32` row, written always or omitted at its default.
fn f32_row(omit_default: bool) -> RowDecl {
    RowDecl { store: Store::F32, omit_default }
}

// ---------------------------------------------------------------------------
// vignette — the first POSITIONED tap (stage 4, D11.1): it measures its own stage
// ---------------------------------------------------------------------------

/// Vignette (R.47): darkening toward the corners, the radius normalized to the half-diagonal of
/// ITS OWN stage; smoothstep written out by hand (one text in both executors).
pub struct Vignette {
    m: Manifest,
}

impl Vignette {
    pub fn new() -> Vignette {
        let p = |n, lo, hi, d| ParamSpec::num(n, lo, hi, d, 0.01, ParamUnit::Norm, ParamKind::Smooth);
        Vignette {
            m: Manifest {
                id: "vignette".into(),
                media: Media::VideoFx,
                passport: vec![p("strength", 0.0, 1.0, 0.5), p("radius", 0.0, 1.5, 0.7), p("softness", 0.05, 1.0, 0.45)],
                rows: vec![f32_row(false), f32_row(false), f32_row(false)],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("vignette.wgsl")),
                    positioned: true,
                    ..Decl::default()
                },
                ui: UiDecl { name: "Vignette", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.vignette", variants: VIGNETTE_VARIANTS, ..UiDecl::default() },
            },
        }
    }

    /// (strength, radius, softness) clamped as the variant's planner clamped them.
    fn numbers(p: &[f64]) -> [f64; 3] {
        let g = |i: usize, d: f64| p.get(i).copied().unwrap_or(d);
        [g(0, 0.5).clamp(0.0, 1.0), g(1, 0.7).clamp(0.0, 1.5), g(2, 0.45).clamp(0.05, 1.0)]
    }
}

impl Default for Vignette {
    fn default() -> Vignette {
        Vignette::new()
    }
}

impl Plugin for Vignette {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn video(&self) -> Option<&dyn VideoKernel> {
        Some(self)
    }
}

impl VideoKernel for Vignette {
    fn cook_at(&self, p: &[f64], cx: &TapCtx) -> Cooked {
        let n = Vignette::numbers(p);
        [n[0], n[1], n[2], cx.stage_w.max(1) as f64, cx.stage_h.max(1) as f64, 0.0, 0.0, 0.0]
    }
    #[inline(always)]
    fn tap_at(&self, k: &Cooked, v: &mut [f64; 4], x: f64, y: f64) {
        let (strength, radius, softness) = (k[0], k[1], k[2]);
        let dx = (x + 0.5) / k[3] - 0.5;
        let dy = (y + 0.5) / k[4] - 0.5;
        let r = ((2.0 * dx) * (2.0 * dx) + (2.0 * dy) * (2.0 * dy)).sqrt() / std::f64::consts::SQRT_2;
        let t = ((r - radius) / softness.max(1e-6)).clamp(0.0, 1.0);
        let w = t * t * (3.0 - 2.0 * t);
        let g = 1.0 - strength * w;
        for c in v.iter_mut().take(3) {
            *c *= g;
        }
    }
    fn gpu_params(&self, p: &[f64]) -> [f32; 7] {
        let n = Vignette::numbers(p);
        // b.zw are the host's: the stage size
        [n[0] as f32, n[1] as f32, n[2] as f32, 0.0, 0.0, 0.0, 0.0]
    }
}

// ---------------------------------------------------------------------------
// picker variants (D7: the manifest names them, kt-app draws them)
// ---------------------------------------------------------------------------

/// The picker's variants of Color Grade (R.54), moved from kt-app's table (plugin-api-stage1, 4.2).
const COLOR_GRADE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.warm", sets: &[("hue", 8.0), ("saturation", 1.15), ("brightness", 0.04)] },
    UiVariant { name_key: "fxreg.var.cool", sets: &[("hue", -12.0), ("saturation", 0.9), ("contrast", 0.1)] },
    UiVariant { name_key: "fxreg.var.bw", sets: &[("saturation", 0.0), ("contrast", 0.15)] },
];

/// The picker's variants of Duotone (R.54), moved from kt-app's table (plugin-api-stage1, 4.2).
const DUOTONE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.sepia", sets: &[("shadow_r", 48.0), ("shadow_g", 28.0), ("shadow_b", 12.0), ("high_r", 255.0), ("high_g", 226.0), ("high_b", 180.0)] },
    UiVariant { name_key: "fxreg.var.blue", sets: &[("shadow_r", 8.0), ("shadow_g", 16.0), ("shadow_b", 64.0), ("high_r", 180.0), ("high_g", 220.0), ("high_b", 255.0)] },
    UiVariant { name_key: "fxreg.var.neon", sets: &[("shadow_r", 32.0), ("shadow_g", 0.0), ("shadow_b", 64.0), ("high_r", 255.0), ("high_g", 64.0), ("high_b", 192.0)] },
];

/// The picker's variants of Posterize (R.54), moved from kt-app's table (plugin-api-stage1, 4.2).
const POSTERIZE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.hard", sets: &[("levels", 3.0)] },
    UiVariant { name_key: "fxreg.var.comic", sets: &[("levels", 4.0)] },
    UiVariant { name_key: "fxreg.var.soft", sets: &[("levels", 12.0)] },
];

/// The picker's variants of Vignette (R.54), moved from kt-app's table (plugin-api-stage1, 4.2).
const VIGNETTE_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.soft", sets: &[("strength", 0.3), ("radius", 0.9), ("softness", 0.6)] },
    UiVariant { name_key: "fxreg.var.strong", sets: &[("strength", 0.9), ("radius", 0.55), ("softness", 0.35)] },
    UiVariant { name_key: "fxreg.var.tight", sets: &[("strength", 0.7), ("radius", 0.4), ("softness", 0.2)] },
];

/// Chroma key (plugin-api-barriers 4.2): a DUAL-POSITION kind (design D4). Inside a fused geometry
/// series it is a TAP — the f64 continuum of the kernel's formula, rounding once at the end; at a
/// series edge it is the integer BARRIER kernel (deliberately scalar, R.32: ×1.29 under the 1.3
/// threshold). It PRODUCES alpha (the alpha rule, R.53 item 417).
pub struct Greenscreen {
    m: Manifest,
}

impl Greenscreen {
    pub fn new() -> Greenscreen {
        let key = |n: &'static str, d: f64| ParamSpec::num(n, 0.0, 255.0, d, 1.0, ParamUnit::Rgb8, ParamKind::Smooth);
        let int = |n: &'static str, lo: f64, hi: f64, d: f64| ParamSpec::num(n, lo, hi, d, 1.0, ParamUnit::None, ParamKind::Smooth);
        Greenscreen {
            m: Manifest {
                id: "greenscreen".into(),
                media: Media::VideoFx,
                passport: vec![
                    key("key_r", 0.0),
                    key("key_g", 255.0),
                    key("key_b", 0.0),
                    int("tolerance", 1.0, 200.0, 70.0),
                    int("softness", 0.0, 100.0, 25.0),
                    ParamSpec::num("spill", 0.0, 1.0, 0.0, 0.01, ParamUnit::Norm, ParamKind::Smooth),
                ],
                rows: vec![
                    RowDecl { store: Store::U8, omit_default: false },
                    RowDecl { store: Store::U8, omit_default: false },
                    RowDecl { store: Store::U8, omit_default: false },
                    RowDecl { store: Store::U32, omit_default: false },
                    RowDecl { store: Store::U32, omit_default: false },
                    RowDecl { store: Store::F64, omit_default: true },
                ],
                decl: Decl {
                    video_class: Some(VideoClass::PointWise),
                    gpu: Gpu::Wgsl(include_str!("tap_greenscreen.wgsl")),
                    alpha: AlphaRel::Produces,
                    ..Decl::default()
                },
                ui: UiDecl {
                    name: "Greenscreen",
                    category_key: "fxreg.cat.key",
                    keywords_key: "fxreg.kw.greenscreen",
                    variants: GREENSCREEN_VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }

    /// `[key_r, key_g, key_b, tolerance, hard, spill, kc]`: tolerance 1..200, softness 0..100,
    /// hard = tolerance − softness (≥ 0), spill 0..1, kc the key's dominant channel.
    fn keyed(p: &[f64]) -> Cooked {
        let key = [p[0] as u8, p[1] as u8, p[2] as u8];
        let tol = (p[3] as u32).clamp(1, 200) as f64;
        let soft = (p[4] as u32).clamp(0, 100) as f64;
        let kc = (0..3).max_by_key(|&i| key[i]).unwrap_or(1);
        [key[0] as f64, key[1] as f64, key[2] as f64, tol, (tol - soft).max(0.0), p[5].clamp(0.0, 1.0), kc as f64, 0.0]
    }
}

const GREENSCREEN_VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.tight", sets: &[("tolerance", 40.0), ("softness", 10.0)] },
    UiVariant { name_key: "fxreg.var.wide", sets: &[("tolerance", 120.0), ("softness", 45.0)] },
    UiVariant { name_key: "fxreg.var.blue", sets: &[("key_g", 0.0), ("key_b", 255.0)] },
];

impl Default for Greenscreen {
    fn default() -> Greenscreen {
        Greenscreen::new()
    }
}

impl Plugin for Greenscreen {
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

impl VideoKernel for Greenscreen {
    fn cook(&self, p: &[f64]) -> Cooked {
        Greenscreen::keyed(p)
    }
    /// The f64 continuum of the kernel's formula on the sub-tap value.
    #[inline(always)]
    fn tap(&self, k: &Cooked, v: &mut [f64; 4]) {
        let (key, tol, hard, spill, kc) = ([k[0], k[1], k[2]], k[3], k[4], k[5], k[6] as usize);
        let d = ((v[0] - key[0]).powi(2) + (v[1] - key[1]).powi(2) + (v[2] - key[2]).powi(2)).sqrt();
        if d <= hard {
            v[3] = 0.0;
        } else if d < tol {
            v[3] *= (d - hard) / (tol - hard).max(1e-9);
        }
        if spill > 0.0 && v[3] > 0.0 {
            let other = v[(kc + 1) % 3].max(v[(kc + 2) % 3]);
            if v[kc] > other {
                v[kc] -= (v[kc] - other) * spill;
            }
        }
    }
    /// a.yzw = the key, b = (tolerance, hard, spill, kc).
    fn gpu_cooked(&self, k: &Cooked, _p: &[f64]) -> [f32; 7] {
        [k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32, k[4] as f32, k[5] as f32, k[6] as f32]
    }
}

impl BarrierKernel for Greenscreen {
    fn cook(&self, p: &[f64]) -> Cooked {
        Greenscreen::keyed(p)
    }

    fn frame(&self, k: &Cooked, mut f: Frame, _cx: &BarrierCtx, _scratch: &mut dyn Scratch) -> Frame {
        let (key, tol, hard, spill, kc) = ([k[0], k[1], k[2]], k[3], k[4], k[5], k[6] as usize);
        for px in f.rgba.chunks_exact_mut(4) {
            let d = ((px[0] as f64 - key[0]).powi(2) + (px[1] as f64 - key[1]).powi(2) + (px[2] as f64 - key[2]).powi(2)).sqrt();
            if d <= hard {
                px[3] = 0;
            } else if d < tol {
                let a = (d - hard) / (tol - hard).max(1e-9);
                px[3] = (px[3] as f64 * a).round() as u8;
            }
            // spill suppression (R.33 C5): the key's dominant channel is pulled toward the larger
            // of the neighbours; spill = 0 — the former path byte-for-byte
            if spill > 0.0 && px[3] > 0 {
                let other = px[(kc + 1) % 3].max(px[(kc + 2) % 3]) as f64;
                let v = px[kc] as f64;
                if v > other {
                    px[kc] = (v - (v - other) * spill).round() as u8;
                }
            }
        }
        f.has_alpha = true;
        f
    }

    fn shaders(&self) -> &'static [&'static str] {
        &[include_str!("greenscreen.wgsl")]
    }

    /// One pass: `u0.x` = kc, `f` = (key, tolerance, hard, spill) in f32.
    fn gpu(&self, k: &Cooked, _g: &GpuCtx, out: &mut Vec<Node>) {
        out.push(Node::Pass {
            shader: 0,
            inputs: [Slot::Prev, Slot::None, Slot::None],
            u: [k[6] as u32, 0, 0, 0, 0, 0, 0, 0],
            f: [k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32, k[4] as f32, k[5] as f32, 0.0, 0.0],
        });
    }
}
