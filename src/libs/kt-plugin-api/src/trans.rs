//! The helpers every transition kernel shares (plugin-api-stage1, D12), moved verbatim from
//! kt-engine's `transitions.rs`: a kernel in a family crate and the core read ONE text of each, and
//! composite.wgsl holds their WGSL mirrors (`samp`, `front_mask`, `t_hash`, `value_noise`, `box9`,
//! `over_straight`). Every per-pixel helper is `#[inline]`: the release profile has no LTO, and a call
//! across the crate boundary would not be inlined otherwise.

/// A frame as a sampling source. A frame without alpha is read as opaque —
/// the same policy as `dissolve_into`. `Clear` — the EMPTY side of a
/// solo fade (transparency): thanks to it, a fade at the track's edge is
/// computed by the same core as a pair, and a wipe “from nothing” works.
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
