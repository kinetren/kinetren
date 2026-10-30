//! The helpers every transition kernel shares (plugin-api-stage1, D12), moved verbatim from
//! kt-engine's `transitions.rs`: a kernel in a family crate and the core read ONE text of each, and
//! composite.wgsl holds their WGSL mirrors (`samp`, `front_mask`, `t_hash`, `value_noise`, `box9`,
//! `over_straight`). Every per-pixel helper is `#[inline]`: the release profile has no LTO, and a call
//! across the crate boundary would not be inlined otherwise.

/// A frame as a sampling source. A frame without alpha is read as opaque —
/// the same policy as `dissolve_into`. `Clear` — the EMPTY side of a
/// solo fade (transparency): thanks to it, a fade at the track's edge is
/// computed by the same core as a pair, and a wipe “from nothing” works.
/// The canvas a transition pixel is computed on: the output's aspect and size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canvas {
    pub aspect: f32,
    pub w: f32,
    pub h: f32,
}

pub enum Img<'a> {
    Frame { px: &'a [u8], w: i32, h: i32, has_alpha: bool },
    Clear,
}

impl Img<'_> {
    #[inline]
    pub fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let Img::Frame { px, w, h, has_alpha } = self else { return [0.0; 4] };
        let x = x.clamp(0, w - 1);
        let y = y.clamp(0, h - 1);
        let i = ((y * w + x) * 4) as usize;
        [
            px[i] as f32,
            px[i + 1] as f32,
            px[i + 2] as f32,
            if *has_alpha { px[i + 3] as f32 } else { 255.0 },
        ]
    }

    /// Bilinear sampling over normalized coordinates (clamp at the edges) —
    /// the same coordinate scheme as `bilinear_into_scalar`.
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let Img::Frame { w, h, .. } = self else { return [0.0; 4] };
        let sx = u * *w as f32 - 0.5;
        let sy = v * *h as f32 - 0.5;
        let x = sx.floor();
        let y = sy.floor();
        let fx = sx - x;
        let fy = sy - y;
        let (x, y) = (x as i32, y as i32);
        let p00 = self.texel(x, y);
        let p10 = self.texel(x + 1, y);
        let p01 = self.texel(x, y + 1);
        let p11 = self.texel(x + 1, y + 1);
        let mut out = [0.0f32; 4];
        for c in 0..4 {
            let top = p00[c] * (1.0 - fx) + p10[c] * fx;
            let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
            out[c] = top * (1.0 - fy) + bot * fy;
        }
        out
    }
}

#[inline]
pub fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut o = [0.0f32; 4];
    for c in 0..4 {
        o[c] = a[c] + (b[c] - a[c]) * t;
    }
    o
}

#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The front of “masking” transitions: 1 — B is already shown, 0 — still A.
/// `s` — the front coordinate 0..1, `k` — the progress 0..1.
#[inline]
pub fn front_mask(s: f32, k: f32, soft: f32) -> f32 {
    let soft = soft.clamp(0.0, 1.0);
    if soft < 1e-4 {
        return if s < k { 1.0 } else { 0.0 };
    }
    // the front moves out beyond both edges by the blur width — otherwise at k=0 and k=1
    // a visible edge would remain
    let front = k * (1.0 + soft) - soft;
    1.0 - smoothstep(front, front + soft, s)
}

/// Integer weight k (0..4096) in fractions.
#[inline]
pub fn kf(k: u32) -> f32 {
    k as f32 / 4096.0
}

/// Hash for the “digital glitch”: the same integer hash as the noise of
/// kt-fx (and it is repeated in WGSL) — otherwise the glitch on CPU and GPU would land
/// in different places and the parity test would catch noise instead of an error.
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn hash01(x: u32) -> f32 {
    (hash_u32(x) >> 8) as f32 / 16_777_216.0
}

/// Panel by the inverse homography (8 numbers from `trans_ext`): screen → (s, t).
/// None — behind the camera (denominator ≤ 0).
#[inline]
pub fn hpanel(h: &[f32], x: f32, y: f32) -> Option<(f32, f32)> {
    let den = h[6] * x + h[7] * y + 1.0;
    if den <= 1e-6 {
        return None;
    }
    Some(((h[0] * x + h[1] * y + h[2]) / den, (h[3] * x + h[4] * y + h[5]) / den))
}

#[inline]
pub fn shade3(mut c: [f32; 4], sh: f32) -> [f32; 4] {
    c[0] *= sh;
    c[1] *= sh;
    c[2] *= sh;
    c
}

/// Coordinate along the curl axis: the front (c = 0) moves TOWARD dir.
#[inline]
pub fn axis_c(u: f32, v: f32, dx: f32, dy: f32) -> f32 {
    if dx > 0.5 {
        1.0 - u
    } else if dx < -0.5 {
        u
    } else if dy > 0.5 {
        1.0 - v
    } else {
        v
    }
}

/// Sample with the AXIAL coordinate substituted (the transverse one is untouched).
#[inline]
pub fn axis_sample(img: &Img<'_>, u: f32, v: f32, dx: f32, dy: f32, m: f32) -> [f32; 4] {
    if dx > 0.5 {
        img.sample(1.0 - m, v)
    } else if dx < -0.5 {
        img.sample(m, v)
    } else if dy > 0.5 {
        img.sample(u, 1.0 - m)
    } else {
        img.sample(u, m)
    }
}

/// The fractional part “as in WGSL”: x − floor(x) (f32::fract has a different sign
/// on negatives — the mirrors would drift apart).
#[inline]
pub fn fracf(x: f32) -> f32 {
    x - x.floor()
}

/// Straight-alpha “top over bottom” in f32 — for the 2.5D exchange, where the
/// sides move separately and real transparency appears.
#[inline]
pub fn over_straight(top: [f32; 4], bot: [f32; 4]) -> [f32; 4] {
    let ta = (top[3] / 255.0).clamp(0.0, 1.0);
    let ba = (bot[3] / 255.0).clamp(0.0, 1.0);
    let oa = ta + ba * (1.0 - ta);
    if oa <= 0.0 {
        return [0.0; 4];
    }
    let mut o = [0.0f32; 4];
    for c in 0..3 {
        o[c] = (top[c] * ta + bot[c] * ba * (1.0 - ta)) / oa;
    }
    o[3] = oa * 255.0;
    o
}

/// Cellular value-noise on the shared integer hash (the same as glitch/blocks)
/// — WGSL repeats it verbatim.
#[inline]
pub fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let h = |i: f32, j: f32| -> f32 {
        let (i, j) = (i as i32 as u32, j as i32 as u32);
        hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)))
    };
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

#[inline]
pub fn quant(x: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return x;
    }
    (x / step).floor() * step + step * 0.5
}

#[allow(clippy::too_many_arguments)]
#[inline]
pub fn rot_sample(
    img: &Img<'_>,
    u: f32,
    v: f32,
    ang: f32,
    zoom: f32,
    aspect: f32,
    ox: f32,
    oy: f32,
) -> [f32; 4] {
    let (s, c) = (ang.sin(), ang.cos());
    let (cx, cy) = ((u - ox) * aspect / zoom, (v - oy) / zoom);
    let rx = cx * c - cy * s;
    let ry = cx * s + cy * c;
    img.sample(rx / aspect + ox, ry + oy)
}

/// A cheap blur with 9 taps (3×3 with step r) — the form is chosen so that
/// it repeats exactly in WGSL.
#[inline]
pub fn box9(img: &Img<'_>, u: f32, v: f32, rx: f32, ry: f32) -> [f32; 4] {
    if rx <= 0.0 && ry <= 0.0 {
        return img.sample(u, v);
    }
    let mut acc = [0.0f32; 4];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let c = img.sample(u + dx as f32 * rx, v + dy as f32 * ry);
            for ch in 0..4 {
                acc[ch] += c[ch];
            }
        }
    }
    for ch in acc.iter_mut() {
        *ch /= 9.0;
    }
    acc
}

/// Inverse homography of the panel: the panel point (s, t) lies in the world as
/// `x(s) = x0 + s·xs`, `z(s) = z0 + s·zs`, `y = t − 0.5`; the camera at
/// (0.5, 0.5, −d). Returns 8 numbers (normalization h22 = 1) of the mapping
/// “screen (X, Y) → panel (s·w, t·w, w)”.
#[inline]
pub fn face_h_inv(x0: f64, xs: f64, z0: f64, zs: f64, d: f64) -> Option<[f32; 8]> {
    // forward matrix (s, t, 1) → (X·w, Y·w, w)
    let m = [
        xs * d + 0.5 * zs,
        0.0,
        (x0 - 0.5) * d + 0.5 * (d + z0),
        0.5 * zs,
        d,
        0.5 * z0,
        zs,
        0.0,
        d + z0,
    ];
    let inv = inv3(m)?;
    let n = inv[8];
    if n.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0f32; 8];
    for i in 0..8 {
        out[i] = (inv[i] / n) as f32;
    }
    Some(out)
}

#[inline]
pub fn inv3(m: [f64; 9]) -> Option<[f64; 9]> {
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if det.abs() < 1e-12 {
        return None;
    }
    let id = 1.0 / det;
    Some([
        (m[4] * m[8] - m[5] * m[7]) * id,
        (m[2] * m[7] - m[1] * m[8]) * id,
        (m[1] * m[5] - m[2] * m[4]) * id,
        (m[5] * m[6] - m[3] * m[8]) * id,
        (m[0] * m[8] - m[2] * m[6]) * id,
        (m[2] * m[3] - m[0] * m[5]) * id,
        (m[3] * m[7] - m[4] * m[6]) * id,
        (m[1] * m[6] - m[0] * m[7]) * id,
        (m[0] * m[4] - m[1] * m[3]) * id,
    ])
}

// ---------------------------------------------------------------------------
// The table-driven kind (D12): a family crate declares each kind as its manifest plus the former
// `TransMix::of` arm (`pack`), `kernel()` arm (`pixel`) and, for a volumetric kind, its `trans_ext`
// arm (`ext`), so the three family crates share one implementation of the plugin traits.
// ---------------------------------------------------------------------------

/// The former `TransMix::of` arm: the passport's values → the eight numbers.
pub type Pack = fn(&[f64]) -> [f32; 8];
/// The former `kernel()` arm.
pub type Pixel = fn(&Img<'_>, &Img<'_>, f32, f32, f32, &[f32; 8], &Canvas, &[f32; 16]) -> [f32; 4];
/// The former `trans_ext` arm: the packed numbers and the progress (0..4096) → the panels.
pub type Ext = fn(&[f32; 8], u32) -> [f32; 16];

/// One transition kind of a family crate.
pub struct Kind {
    m: crate::Manifest,
    pack: Pack,
    pixel: Pixel,
    ext: Option<Ext>,
    symmetric: bool,
}

impl Kind {
    /// The volumetric kinds' panels.
    pub fn with_ext(mut self, ext: Ext) -> Kind {
        self.ext = Some(ext);
        self
    }
}

impl crate::Plugin for Kind {
    fn manifest(&self) -> &crate::Manifest {
        &self.m
    }
    fn trans(&self) -> Option<&dyn crate::TransKernel> {
        Some(self)
    }
}

impl crate::TransKernel for Kind {
    fn pack(&self, v: &[f64]) -> [f32; 8] {
        (self.pack)(v)
    }
    fn pixel(&self, a: &Img<'_>, b: &Img<'_>, u: f32, v: f32, k: f32, p: &[f32; 8], cx: &Canvas, ext: &[f32; 16]) -> [f32; 4] {
        (self.pixel)(a, b, u, v, k, p, cx, ext)
    }
    fn ext(&self, p: &[f32; 8], k: u32) -> [f32; 16] {
        self.ext.map_or([0.0; 16], |e| e(p, k))
    }
    fn symmetric(&self) -> bool {
        self.symmetric
    }
}

/// A passport row of a kind with its storage. The passport lists the rows in the former variant's
/// FIELD order (the file writes them in that order).
pub enum Row {
    /// A number row, always written.
    Num(crate::ParamSpec, crate::Store),
    /// A centre coordinate (0..1), omitted at 0.5 (the former `skip_serializing_if = "is_center"`).
    Center(&'static str),
    /// The direction (`DIRS`).
    Dir,
    /// A choice row with its keys.
    Choice(&'static str, &'static [&'static str]),
    /// A flag row (default off).
    Flag(&'static str),
}

/// The direction row's keys (the file holds the former `Dir` text).
pub const DIRS: &[&str] = &["left", "right", "up", "down"];

/// A smooth number row.
pub fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: crate::ParamUnit, store: crate::Store) -> Row {
    Row::Num(crate::ParamSpec::num(n, lo, hi, d, st, u, crate::ParamKind::Smooth), store)
}

/// The `softness` row (0..1).
pub fn soft(d: f64) -> Row {
    smooth("softness", 0.0, 1.0, d, 0.01, crate::ParamUnit::Norm, crate::Store::F64)
}

/// A colour component row (`u8`, 0..255).
pub fn rgb(n: &'static str, d: f64) -> Row {
    smooth(n, 0.0, 255.0, d, 1.0, crate::ParamUnit::Rgb8, crate::Store::U8)
}

/// A count row (`u32`).
pub fn count(n: &'static str, lo: f64, hi: f64, d: f64) -> Row {
    smooth(n, lo, hi, d, 1.0, crate::ParamUnit::Count, crate::Store::U32)
}

/// A kind: its id (the file's tag), English name, name and category catalog keys, whether it is its
/// own inversion, the rows, its WGSL (`trans_<id>`) and the two arms.
#[allow(clippy::too_many_arguments)]
pub fn kind(
    id: &'static str,
    name: &'static str,
    name_key: &'static str,
    category: &'static str,
    symmetric: bool,
    rows: Vec<Row>,
    wgsl: &'static str,
    pack: Pack,
    pixel: Pixel,
) -> Kind {
    use crate::{ParamKind, ParamSpec, ParamUnit, RowDecl, Store};
    let mut passport = Vec::new();
    let mut decls = Vec::new();
    for r in rows {
        let (spec, store, omit) = match r {
            Row::Num(s, st) => (s, st, false),
            Row::Center(n) => {
                (ParamSpec::num(n, 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm, ParamKind::Smooth), Store::F64, true)
            }
            Row::Dir => (ParamSpec::choice_row("dir", DIRS, 0), Store::Choice(DIRS), false),
            Row::Choice(n, keys) => (ParamSpec::choice_row(n, keys, 0), Store::Choice(keys), false),
            Row::Flag(n) => (ParamSpec::flag_row(n, false), Store::Bool, false),
        };
        passport.push(spec);
        decls.push(RowDecl { store, omit_default: omit });
    }
    Kind {
        m: crate::Manifest {
            id: id.into(),
            media: crate::Media::Transition,
            passport,
            rows: decls,
            decl: crate::Decl { gpu: crate::Gpu::Wgsl(wgsl), ..crate::Decl::default() },
            ui: crate::UiDecl { name, category_key: category, name_key, ..crate::UiDecl::default() },
        },
        pack,
        pixel,
        ext: None,
        symmetric,
    }
}

/// The unit vector of a direction row's index in frame coordinates (X right, Y down) — `Dir::vec`.
#[inline]
pub fn dir_vec(i: f64) -> (f32, f32) {
    match i as u32 {
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        3 => (0.0, 1.0),
        _ => (-1.0, 0.0),
    }
}

/// A flag row's value as the uniform's 0/1.
#[inline]
pub fn flag(v: f64) -> f32 {
    if v > 0.5 { 1.0 } else { 0.0 }
}

/// The values as they are (the kinds whose numbers map one to one onto the uniform).
pub fn direct(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    for (o, x) in p.iter_mut().zip(v) {
        *o = *x as f32;
    }
    p
}

/// A kind whose only row is the direction: its vector in `p[0..2]`.
pub fn dir_pack(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    (p[0], p[1]) = dir_vec(v[0]);
    p
}

/// A kind whose only row is a flag: `p[0]`.
pub fn flag_pack(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    p[0] = flag(v[0]);
    p
}
