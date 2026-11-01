//! The CPU kernel: the byte reference (invariant 7). Six passes over u32 texels (RGBA8 packed
//! little-endian, the GPU's Rgba8Uint), quantized at exactly the points the GPU textures quantize
//! (design D1). Each pass is a row function over a pure per-pixel text in f32; `p*.wgsl` mirror it
//! line by line — an edit here is an edit there, in the same commit (t1217).
//!
//! The sensor works on its own pixel grid (D6): output pixel (x, y) is sensor pixel (x/K, y/K), and a
//! neighbour is read at its block's top-left texel.

use std::cell::RefCell;

use kt_plugin_api::barrier::{Frame, Scratch, noise_hash as h};
use kt_plugin_api::math::FrozenMath;

use crate::cfa::{self, IR};
use crate::{DYE_NIR, IR_FROM_RGB, IR_VIS_LEAK, Plan};

/// 1/σ of Irwin–Hall(4) over 16-bit fields: σ = 65536·√(1/3) (design D4). The WGSL literal is the same.
pub const IH_INV_SD: f32 = 2.642_896_6e-5;
/// The 16-bit fixed point of the linear passes: 1.0 = 16384 (0…4).
const FIX: f32 = 16384.0;
const INV_FIX: f32 = 1.0 / 16384.0;
const INV_255: f32 = 0.003_921_569;
/// The chroma NR's luma guide: 1/0.06².
const CHROMA_GUIDE: f32 = 277.777_78;
/// The edge-directed demosaic's floor of the gradient.
const EDGE_EPS: f32 = 1.0 / 256.0;

// ---- the shared numerics (mirrored by camera_common.wgsl) ----------------------------------------

/// A counter hash of a sensor pixel and a stream seed.
#[inline(always)]
pub fn hash2(x: u32, y: u32, s: u32) -> u32 {
    h(x ^ h(y ^ s))
}

/// A normal deviate of a hash: Irwin–Hall(4) of four 16-bit fields of two hashes (integer sums, one
/// multiply — the same bits on every device).
#[inline(always)]
pub fn gauss(h1: u32) -> f32 {
    let h2 = h(h1.wrapping_add(0x9E37_79B9));
    let s = (h1 & 0xFFFF) + (h1 >> 16) + (h2 & 0xFFFF) + (h2 >> 16);
    (s as f32 - 131_070.0) * IH_INV_SD
}

/// A uniform in [0, 1) of a hash (24 bits, exact in f32).
#[inline(always)]
pub fn unit(h1: u32) -> f32 {
    (h1 >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// A Poisson count of mean λ: the inverse CDF below 12, the normal approximation above (D4).
#[inline(always)]
pub fn poisson(lambda: f32, h1: u32) -> f32 {
    if lambda < 12.0 {
        let u = unit(h1);
        let mut p = (-lambda).fexp();
        let mut c = p;
        let mut k = 0.0f32;
        while u > c && k < 64.0 {
            k += 1.0;
            p = p * lambda / k;
            c += p;
        }
        k
    } else {
        (lambda + lambda.sqrt() * gauss(h1) + 0.5).floor().max(0.0)
    }
}

/// sRGB decode of a byte (the scene's linearisation, design step 1).
#[inline(always)]
pub fn srgb_lin(c: u32) -> f32 {
    let v = c as f32 * INV_255;
    if v <= 0.04045 { v * 0.077_399_38 } else { ((v + 0.055) * 0.947_867_3).fpowf(2.4) }
}

/// The tone curve (design step 20): 0 linear, 1 sRGB, 2 BT.709, 3 gamma 2.2; clipped to [0, 1].
#[inline(always)]
pub fn oetf(v: f32, curve: u32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    match curve {
        1 => {
            if v <= 0.003_130_8 {
                v * 12.92
            } else {
                1.055 * v.fpowf(0.416_666_66) - 0.055
            }
        }
        2 => {
            if v < 0.018 {
                v * 4.5
            } else {
                1.099 * v.fpowf(0.45) - 0.099
            }
        }
        3 => {
            if v > 0.0 {
                v.fpowf(0.454_545_45)
            } else {
                0.0
            }
        }
        _ => v,
    }
}

#[inline(always)]
fn byte(v: f32) -> u32 {
    (v * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u32
}

#[inline(always)]
fn fix16(v: f32) -> u32 {
    (v * FIX + 0.5).floor().clamp(0.0, 65535.0) as u32
}

#[inline(always)]
fn texel(r: u32, g: u32, b: u32, a: u32) -> u32 {
    r | g << 8 | b << 16 | a << 24
}

/// A 16-bit value of a texel (R high, G low).
#[inline(always)]
fn v16(t: u32) -> u32 {
    (t & 0xFF) << 8 | (t >> 8) & 0xFF
}

#[inline(always)]
fn hi_lo(v: u32) -> (u32, u32) {
    (v >> 8, v & 0xFF)
}

// ---- the context of one pass --------------------------------------------------------------------

/// What a row function reads: the plan, the frame and sensor sizes, the source bytes and the
/// previous pass.
pub struct Ctx<'a> {
    pub c: &'a Plan,
    pub w: u32,
    pub h: u32,
    /// The sensor's size (the logical frame, D6).
    pub sw: i32,
    pub sh: i32,
    pub src: &'a [u8],
    pub inp: &'a [u32],
    pub lut: &'a [f32; 256],
}

impl Ctx<'_> {
    /// The previous pass at sensor pixel (sx, sy), clamped.
    #[inline(always)]
    fn at(&self, sx: i32, sy: i32) -> u32 {
        let k = self.c.k as i32;
        let x = (sx.clamp(0, self.sw - 1) * k).min(self.w as i32 - 1) as usize;
        let y = (sy.clamp(0, self.sh - 1) * k).min(self.h as i32 - 1) as usize;
        self.inp[y * self.w as usize + x]
    }

    /// A neighbour of the mosaic: outside the frame it is shifted by the layout's period, which keeps
    /// its colour (D7).
    #[inline(always)]
    fn wrap(&self, sx: i32, sy: i32) -> (i32, i32) {
        let p = self.c.period as i32;
        let f = |n: i32, size: i32| -> i32 {
            let mut n = n;
            if n < 0 {
                n += p;
            }
            if n >= size {
                n -= p;
            }
            n.clamp(0, size - 1)
        };
        (f(sx, self.sw), f(sy, self.sh))
    }

    #[inline(always)]
    fn src_alpha(&self, x: u32, y: u32) -> u32 {
        self.src[((y * self.w + x) * 4 + 3) as usize] as u32
    }

    /// The squared radius of a sensor pixel's centre over the squared half-diagonal.
    #[inline(always)]
    fn r2(&self, sx: i32, sy: i32) -> f32 {
        let (hw, hh) = (self.sw as f32 * 0.5, self.sh as f32 * 0.5);
        let cx = sx as f32 + 0.5 - hw;
        let cy = sy as f32 + 0.5 - hh;
        (cx * cx + cy * cy) / (hw * hw + hh * hh)
    }

    /// One channel of the scene's linear RGB, bilinear at (px, py) in SENSOR pixel units.
    #[inline(always)]
    fn scene(&self, px: f32, py: f32, c: usize) -> f32 {
        let k = self.c.k as f32;
        let fx = px * k - 0.5;
        let fy = py * k - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (wm, hm) = (self.w as i32 - 1, self.h as i32 - 1);
        let xa = (x0 as i32).clamp(0, wm) as u32;
        let xb = (x0 as i32 + 1).clamp(0, wm) as u32;
        let ya = (y0 as i32).clamp(0, hm) as u32;
        let yb = (y0 as i32 + 1).clamp(0, hm) as u32;
        let s = |x: u32, y: u32| self.lut[self.src[((y * self.w + x) * 4) as usize + c] as usize];
        let top = s(xa, ya) + (s(xb, ya) - s(xa, ya)) * tx;
        let bot = s(xa, yb) + (s(xb, yb) - s(xa, yb)) * tx;
        top + (bot - top) * ty
    }
}

// ---- P0: optics and photons ---------------------------------------------------------------------

/// The scene through the lens at a sensor pixel: CA per channel, the 4-spot OLPF, cos⁴ vignetting.
#[inline(always)]
fn optics(cx: &Ctx, sx: i32, sy: i32) -> [f32; 3] {
    let c = cx.c;
    let (px, py) = (sx as f32 + 0.5, sy as f32 + 0.5);
    if !c.optics {
        return [cx.scene(px, py, 0), cx.scene(px, py, 1), cx.scene(px, py, 2)];
    }
    let (hw, hh) = (cx.sw as f32 * 0.5, cx.sh as f32 * 0.5);
    let (dx, dy) = (px - hw, py - hh);
    let ca = c.ca / (hw * hw + hh * hh).sqrt();
    let d = c.olpf * 0.5;
    let mut out = [0.0f32; 3];
    for (ch, o) in out.iter_mut().enumerate() {
        let s = 1.0 + ca * (1.0 - ch as f32);
        let (qx, qy) = (hw + dx * s, hh + dy * s);
        *o = if d > 0.0 {
            let a = cx.scene(qx - d, qy - d, ch);
            let b = cx.scene(qx + d, qy - d, ch);
            let e = cx.scene(qx - d, qy + d, ch);
            let f = cx.scene(qx + d, qy + d, ch);
            (a + b + e + f) * 0.25
        } else {
            cx.scene(qx, qy, ch)
        };
    }
    let t = 1.0 + c.vig_t2 * cx.r2(sx, sy);
    let e = 1.0 / (t * t);
    [out[0] * e, out[1] * e, out[2] * e]
}

/// The mean signal of a sensor pixel as a fraction of the full well (design steps 7–9).
#[inline(always)]
fn photons(c: &Plan, code: u32, s: [f32; 3]) -> f32 {
    let [r, g, b] = s;
    let x = c.xtalk;
    let o = 1.0 - x;
    let hx = x * 0.5;
    let ir = c.scene_ir * (IR_FROM_RGB[0] * r + IR_FROM_RGB[1] * g + IR_FROM_RGB[2] * b);
    let vis = match code {
        0 => c.a[0] * (o * r + hx * (g + b)),
        1 => c.a[1] * (o * g + hx * (r + b)),
        2 => c.a[2] * (o * b + hx * (r + g)),
        3 => c.a[0] * r + c.a[1] * g + c.a[2] * b,
        _ => IR_VIS_LEAK * (c.a[0] * r + c.a[1] * g + c.a[2] * b),
    };
    vis + c.cir * DYE_NIR[code as usize] * ir
}

#[inline(always)]
pub(crate) fn p0_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let k = cx.c.k;
    let sy = (y / k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / k) as i32;
        let code = cfa::code(&cx.c.sensor_tab, cx.c.period, sx, sy);
        let mu = photons(cx.c, code, optics(cx, sx, sy));
        let (hi, lo) = hi_lo(fix16(mu));
        *o = texel(hi, lo, 0, 255);
    }
}

// ---- P1: the sensor's electronics ---------------------------------------------------------------

#[inline(always)]
pub(crate) fn p1_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let w = cx.w as usize;
    let sy = y / c.k;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = x as u32 / c.k;
        let lam_s = v16(cx.inp[y as usize * w + x]) as f32 * INV_FIX * c.fw;
        let hf = hash2(sx, sy, c.seed_f);
        let dn = if hf < c.dead_thr {
            c.black as f32
        } else {
            let gpx = 1.0 + c.prnu * gauss(hash2(sx, sy, c.seed_f ^ 0x1111_1111));
            let mut dk = (c.dark + c.dsnu * gauss(hash2(sx, sy, c.seed_f ^ 0x2222_2222))).max(0.0);
            if hf - c.dead_thr < c.hot_thr {
                dk += c.fw * (0.2 + 0.8 * unit(h(hf)));
            }
            let lam = lam_s * gpx + dk;
            let n = if c.noise {
                let n = poisson(lam, hash2(sx, sy, c.seed_t)).min(c.fw);
                n + c.read * gauss(hash2(sx, sy, c.seed_t ^ 0x5555_5555))
            } else {
                lam.min(c.fw)
            };
            (c.black as f32 + n * c.cg + 0.5).floor().clamp(0.0, c.adc_max as f32)
        };
        let (hi, lo) = hi_lo(dn as u32);
        *o = texel(hi, lo, 0, 255);
    }
}

// ---- P2: the raw front --------------------------------------------------------------------------

#[inline(always)]
pub(crate) fn p2_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let tab = &c.isp_tab;
    let vb = |sx: i32, sy: i32| (v16(cx.at(sx, sy)) as f32 - c.b_isp) * c.inv_range;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let code = cfa::code(tab, c.period, sx, sy);
        let mut v = vb(sx, sy);
        if c.dpc {
            let (mut mn, mut mx, mut any) = (f32::MAX, f32::MIN, false);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    if (dx == 0 && dy == 0) || cfa::code(tab, c.period, sx + dx, sy + dy) != code {
                        continue;
                    }
                    let (nx, ny) = cx.wrap(sx + dx, sy + dy);
                    let q = vb(nx, ny);
                    mn = mn.min(q);
                    mx = mx.max(q);
                    any = true;
                }
            }
            if any {
                if v > mx + c.dpc_thr {
                    v = mx;
                } else if v < mn - c.dpc_thr {
                    v = mn;
                }
            }
        }
        if c.ir_sub > 0.0 && code != IR {
            let (mut s, mut n) = (0.0f32, 0.0f32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if cfa::code(tab, c.period, sx + dx, sy + dy) == IR {
                        let (nx, ny) = cx.wrap(sx + dx, sy + dy);
                        s += vb(nx, ny);
                        n += 1.0;
                    }
                }
            }
            if n > 0.0 {
                v -= c.ir_sub * (DYE_NIR[code as usize] / DYE_NIR[IR as usize]) * (s / n);
            }
        }
        if c.lsc_pow > 0.0 {
            v *= (1.0 + c.vig_t2 * cx.r2(sx, sy)).fpowf(c.lsc_pow);
        }
        if code < 4 {
            v *= c.gains[code as usize];
        }
        let (hi, lo) = hi_lo(fix16(v.max(0.0)));
        *o = texel(hi, lo, 0, 255);
    }
}

// ---- P3: demosaic, CCM, tone --------------------------------------------------------------------

#[inline(always)]
fn vfix(cx: &Ctx, sx: i32, sy: i32) -> f32 {
    let (nx, ny) = cx.wrap(sx, sy);
    v16(cx.at(nx, ny)) as f32 * INV_FIX
}

/// The mean of the neighbours of code `want` within `r` (self excluded); (sum, count).
#[inline(always)]
fn near(cx: &Ctx, sx: i32, sy: i32, want: u32, r: i32) -> (f32, f32) {
    let c = cx.c;
    let (mut s, mut n) = (0.0f32, 0.0f32);
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx != 0 || dy != 0) && cfa::code(&c.isp_tab, c.period, sx + dx, sy + dy) == want {
                s += vfix(cx, sx + dx, sy + dy);
                n += 1.0;
            }
        }
    }
    (s, n)
}

#[inline(always)]
fn bilinear(cx: &Ctx, sx: i32, sy: i32, want: u32) -> f32 {
    let (mut s, mut n) = near(cx, sx, sy, want, 1);
    if n == 0.0 {
        (s, n) = near(cx, sx, sy, want, 2);
    }
    if n > 0.0 { s / n } else { 0.0 }
}

/// The green at a site, edge-directed (Hamilton–Adams): each green neighbour weighted by the inverse
/// gradient across the site along its direction — the first difference of the neighbours and the
/// second difference of the site's own colour.
#[inline(always)]
fn gest(cx: &Ctx, sx: i32, sy: i32) -> f32 {
    let c = cx.c;
    let v0 = vfix(cx, sx, sy);
    if cfa::code(&c.isp_tab, c.period, sx, sy) == cfa::G {
        return v0;
    }
    for r in 1..=2 {
        let (mut s, mut sw) = (0.0f32, 0.0f32);
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx == 0 && dy == 0)
                    || cfa::code(&c.isp_tab, c.period, sx + dx, sy + dy) != cfa::G
                {
                    continue;
                }
                let (ux, uy) = (dx.signum(), dy.signum());
                let g1 = (vfix(cx, sx + ux, sy + uy) - vfix(cx, sx - ux, sy - uy)).abs();
                let g2 = (2.0 * v0
                    - vfix(cx, sx + 2 * ux, sy + 2 * uy)
                    - vfix(cx, sx - 2 * ux, sy - 2 * uy))
                .abs();
                let wt = 1.0 / (EDGE_EPS + g1 + g2);
                s += wt * vfix(cx, sx + dx, sy + dy);
                sw += wt;
            }
        }
        if sw > 0.0 {
            return s / sw;
        }
    }
    v0
}

/// Edge-directed demosaic: the green by `gest`, red and blue by the colour difference to green of
/// their nearest sites — the difference is smooth across an edge, the colour is not (no zipper).
#[inline(always)]
fn edge(cx: &Ctx, sx: i32, sy: i32, want: u32) -> f32 {
    let gp = gest(cx, sx, sy);
    if want == cfa::G {
        return gp;
    }
    let c = cx.c;
    for r in 1..=2 {
        let (mut s, mut n) = (0.0f32, 0.0f32);
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx != 0 || dy != 0) && cfa::code(&c.isp_tab, c.period, sx + dx, sy + dy) == want
                {
                    s += vfix(cx, sx + dx, sy + dy) - gest(cx, sx + dx, sy + dy);
                    n += 1.0;
                }
            }
        }
        if n > 0.0 {
            return gp + s / n;
        }
    }
    gp
}

/// The direction-weighted interpolation of a colour itself (the edge-directed method on layouts that
/// are not a 2×2 Bayer, where green is too sparse for colour differences): each same-colour
/// neighbour weighted by the inverse gradient across the site along its direction.
#[inline(always)]
fn directional(cx: &Ctx, sx: i32, sy: i32, want: u32, v0: f32) -> f32 {
    let c = cx.c;
    for r in 1..=2 {
        let (mut s, mut sw) = (0.0f32, 0.0f32);
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx == 0 && dy == 0) || cfa::code(&c.isp_tab, c.period, sx + dx, sy + dy) != want
                {
                    continue;
                }
                let (ux, uy) = (dx.signum(), dy.signum());
                let g1 = (vfix(cx, sx + ux, sy + uy) - vfix(cx, sx - ux, sy - uy)).abs();
                let g2 = (2.0 * v0
                    - vfix(cx, sx + 2 * ux, sy + 2 * uy)
                    - vfix(cx, sx - 2 * ux, sy - 2 * uy))
                .abs();
                let wt = 1.0 / (EDGE_EPS + g1 + g2);
                s += wt * vfix(cx, sx + dx, sy + dy);
                sw += wt;
            }
        }
        if sw > 0.0 {
            return s / sw;
        }
    }
    0.0
}

/// Malvar–He–Cutler on a 2×2 Bayer: the exact 5×5 kernels.
#[inline(always)]
fn mhc_bayer(cx: &Ctx, sx: i32, sy: i32, code: u32, want: u32) -> f32 {
    let v = |dx: i32, dy: i32| vfix(cx, sx + dx, sy + dy);
    let v0 = v(0, 0);
    let axial2 = v(-2, 0) + v(2, 0) + v(0, -2) + v(0, 2);
    let diag = v(-1, -1) + v(1, -1) + v(-1, 1) + v(1, 1);
    if want == 1 {
        // G at R or B
        return (4.0 * v0 + 2.0 * (v(-1, 0) + v(1, 0) + v(0, -1) + v(0, 1)) - axial2) * 0.125;
    }
    if code == 1 {
        // R or B at G: the colour of the horizontal neighbours, or of the vertical ones
        let horiz = cfa::code(&cx.c.isp_tab, cx.c.period, sx + 1, sy) == want;
        let (a1, a2, b2) = if horiz {
            (v(-1, 0) + v(1, 0), v(-2, 0) + v(2, 0), v(0, -2) + v(0, 2))
        } else {
            (v(0, -1) + v(0, 1), v(0, -2) + v(0, 2), v(-2, 0) + v(2, 0))
        };
        return (5.0 * v0 + 4.0 * a1 - a2 - diag + 0.5 * b2) * 0.125;
    }
    // B at R, R at B
    (6.0 * v0 + 2.0 * diag - 1.5 * axial2) * 0.125
}

#[inline(always)]
pub(crate) fn p3_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let code = cfa::code(&c.isp_tab, c.period, sx, sy);
        let v0 = vfix(cx, sx, sy);
        let mut rgb = [0.0f32; 3];
        for (want, out_c) in rgb.iter_mut().enumerate() {
            let want = want as u32;
            *out_c = if want == code {
                v0
            } else if c.method == 0 {
                bilinear(cx, sx, sy, want)
            } else if c.method == 1 {
                if c.bayer {
                    mhc_bayer(cx, sx, sy, code, want)
                } else if code == IR {
                    bilinear(cx, sx, sy, want)
                } else {
                    let (s, n) = near(cx, sx, sy, code, 2);
                    let own = if n > 0.0 { s / n } else { v0 };
                    bilinear(cx, sx, sy, want) + 0.5 * (v0 - own)
                }
            } else if c.bayer {
                edge(cx, sx, sy, want)
            } else {
                directional(cx, sx, sy, want, v0)
            };
        }
        let m = &c.ccm;
        let r = m[0] * rgb[0] + m[1] * rgb[1] + m[2] * rgb[2];
        let g = m[3] * rgb[0] + m[4] * rgb[1] + m[5] * rgb[2];
        let b = m[6] * rgb[0] + m[7] * rgb[1] + m[8] * rgb[2];
        *o = texel(
            byte(oetf(r, c.tone)),
            byte(oetf(g, c.tone)),
            byte(oetf(b, c.tone)),
            cx.src_alpha(x as u32, y),
        );
    }
}

// ---- P4: YUV, denoise, sharpening ---------------------------------------------------------------

#[inline(always)]
fn yuv(cx: &Ctx, sx: i32, sy: i32) -> [f32; 3] {
    let t = cx.at(sx, sy);
    let e = &cx.c.enc;
    let r = (t & 0xFF) as f32 * INV_255;
    let g = (t >> 8 & 0xFF) as f32 * INV_255;
    let b = (t >> 16 & 0xFF) as f32 * INV_255;
    let yy = e[0] * r + e[1] * g + e[2] * b;
    [yy, (b - yy) * e[3], (r - yy) * e[4]]
}

/// RGB of Y'CbCr by a decoder (Kr, Kb, 1/Kg, 2(1−Kr), 2(1−Kb)).
#[inline(always)]
fn rgb_of(d: &[f32; 5], yy: f32, u: f32, v: f32) -> [f32; 3] {
    let r = yy + d[3] * v;
    let b = yy + d[4] * u;
    let g = (yy - d[0] * r - d[1] * b) * d[2];
    [r, g, b]
}

#[inline(always)]
pub(crate) fn p4_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let [y0, mut u, mut v] = yuv(cx, sx, sy);
        let mut ynr = y0;
        if c.luma_inv_s2 > 0.0 {
            let (mut s, mut sw) = (0.0f32, 0.0f32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = yuv(cx, sx + dx, sy + dy)[0];
                    let d = q - y0;
                    let wt = 1.0 / (1.0 + d * d * c.luma_inv_s2);
                    s += wt * q;
                    sw += wt;
                }
            }
            ynr = s / sw;
        }
        if c.chroma_amt > 0.0 {
            let (mut su, mut sv, mut sw) = (0.0f32, 0.0f32, 0.0f32);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let q = yuv(cx, sx + dx, sy + dy);
                    let d = q[0] - y0;
                    let wt = 1.0 / (1.0 + d * d * CHROMA_GUIDE);
                    su += wt * q[1];
                    sv += wt * q[2];
                    sw += wt;
                }
            }
            u += c.chroma_amt * (su / sw - u);
            v += c.chroma_amt * (sv / sw - v);
        }
        let mut yo = ynr;
        if c.sharpen > 0.0 {
            let mut s = 0.0f32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    s += yuv(cx, sx + dx, sy + dy)[0];
                }
            }
            yo = ynr + c.sharpen * (y0 - s * 0.111_111_11);
        }
        let a = cx.src_alpha(x as u32, y);
        *o = if c.yuv {
            texel(byte(yo), byte(u + 0.5), byte(v + 0.5), a)
        } else {
            let [r, g, b] = rgb_of(&c.dec_same, yo, u, v);
            texel(byte(r), byte(g), byte(b), a)
        };
    }
}

// ---- P5: chroma subsampling and the decoder ------------------------------------------------------

#[inline(always)]
pub(crate) fn p5_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let t = cx.at(sx, sy);
        let (bx, by, bw, bh) = match c.chroma {
            1 => (sx & !1, sy, 2, 1),
            2 => (sx & !1, sy & !1, 2, 2),
            _ => (sx, sy, 1, 1),
        };
        let (mut su, mut sv, mut n) = (0u32, 0u32, 0u32);
        for dy in 0..bh {
            for dx in 0..bw {
                if bx + dx < cx.sw && by + dy < cx.sh {
                    let q = cx.at(bx + dx, by + dy);
                    su += q >> 8 & 0xFF;
                    sv += q >> 16 & 0xFF;
                    n += 1;
                }
            }
        }
        let u8_ = (su + n / 2) / n;
        let v8_ = (sv + n / 2) / n;
        let yy = (t & 0xFF) as f32 * INV_255;
        let u = u8_ as f32 * INV_255 - 0.5;
        let v = v8_ as f32 * INV_255 - 0.5;
        let [r, g, b] = rgb_of(&c.dec, yy, u, v);
        *o = texel(byte(r), byte(g), byte(b), cx.src_alpha(x as u32, y));
    }
}

// ---- the driver ---------------------------------------------------------------------------------

type Row = fn(&Ctx, u32, &mut [u32]);

thread_local! {
    /// The two ping-pong planes, reused per thread (invariant 12: no allocation per frame once warm).
    static PLANES: RefCell<(Vec<u32>, Vec<u32>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// The sRGB decode table, built once with the frozen pow (D5).
fn lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| std::array::from_fn(|i| srgb_lin(i as u32)))
}

/// One pass over all rows, split across scoped threads (rows are independent: no effect on bytes).
fn pass(cx: &Ctx, out: &mut [u32], row: Row, twin: Option<Row>) {
    let w = cx.w as usize;
    let f = twin.unwrap_or(row);
    let threads = if cx.h < 32 {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get()).min(8)
    };
    if threads <= 1 {
        for (y, r) in out.chunks_exact_mut(w).enumerate() {
            f(cx, y as u32, r);
        }
        return;
    }
    let rows_per = (cx.h as usize).div_ceil(threads);
    std::thread::scope(|s| {
        for (i, chunk) in out.chunks_mut(rows_per * w).enumerate() {
            s.spawn(move || {
                for (j, r) in chunk.chunks_exact_mut(w).enumerate() {
                    f(cx, (i * rows_per + j) as u32, r);
                }
            });
        }
    });
}

/// The whole camera on the CPU: P0…P5, the frame handed back with the output bytes.
pub fn run(plan: &Plan, mut f: Frame, simd: bool, _scratch: &mut dyn Scratch) -> Frame {
    let (w, h) = (f.w, f.h);
    if w == 0 || h == 0 {
        return f;
    }
    let n = (w * h) as usize;
    let k = plan.k;
    let (sw, sh) = (w.div_ceil(k) as i32, h.div_ceil(k) as i32);
    #[cfg(target_arch = "x86_64")]
    let twins: [Option<Row>; 6] = if simd { crate::simd::TWINS.map(Some) } else { [None; 6] };
    #[cfg(not(target_arch = "x86_64"))]
    let twins: [Option<Row>; 6] = {
        let _ = simd;
        [None; 6]
    };
    let rows: [Row; 6] = [p0_row, p1_row, p2_row, p3_row, p4_row, p5_row];
    PLANES.with(|planes| {
        let mut planes = planes.borrow_mut();
        let (a, b) = &mut *planes;
        a.resize(n, 0);
        b.resize(n, 0);
        let empty: [u32; 0] = [];
        let src = &f.rgba;
        let lut = lut();
        macro_rules! ctx {
            ($inp:expr) => {
                Ctx { c: plan, w, h, sw, sh, src, inp: $inp, lut }
            };
        }
        pass(&ctx!(&empty), a, rows[0], twins[0]);
        pass(&ctx!(a), b, rows[1], twins[1]);
        pass(&ctx!(b), a, rows[2], twins[2]);
        pass(&ctx!(a), b, rows[3], twins[3]);
        let mut last_is_b = true;
        if plan.p4 {
            pass(&ctx!(b), a, rows[4], twins[4]);
            last_is_b = false;
            if plan.yuv {
                pass(&ctx!(a), b, rows[5], twins[5]);
                last_is_b = true;
            }
        }
        let last: &[u32] = if last_is_b { b } else { a };
        for (px, t) in f.rgba.chunks_exact_mut(4).zip(last) {
            px.copy_from_slice(&t.to_le_bytes());
        }
    });
    f
}

/// The plane after pass `upto` (0 = P0 … 5 = P5), for tests that read the raw (t1212). Allocates.
pub fn plane_after(plan: &Plan, f: &Frame, upto: usize) -> Vec<u32> {
    let (w, h) = (f.w, f.h);
    let n = (w * h) as usize;
    let k = plan.k;
    let (sw, sh) = (w.div_ceil(k) as i32, h.div_ceil(k) as i32);
    let rows: [Row; 6] = [p0_row, p1_row, p2_row, p3_row, p4_row, p5_row];
    let mut prev = vec![0u32; n];
    for (i, row) in rows.iter().enumerate().take(upto + 1) {
        let mut out = vec![0u32; n];
        let cx = Ctx { c: plan, w, h, sw, sh, src: &f.rgba, inp: &prev, lut: lut() };
        pass(&cx, &mut out, *row, None);
        prev = out;
        let _ = i;
    }
    prev
}
