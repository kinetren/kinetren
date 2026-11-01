//! The CPU kernel: the byte reference (invariant 7). Up to twelve passes over u32 texels (RGBA8
//! packed little-endian, the GPU's Rgba8Uint), quantized at exactly the points the GPU textures
//! quantize (design D1). Each pass is a row function over a pure per-pixel text in f32; the
//! `sNN_*.wgsl` passes mirror them line by line — an edit here is an edit there, in the same commit
//! (t1217).
//!
//! The sensor works on its own pixel grid (D6): output pixel (x, y) is sensor pixel (x/K, y/K), and a
//! neighbour is read at its block's top-left texel. The unprocessing alone works at frame pixels:
//! the scene is continuous.

use std::cell::RefCell;

use kt_plugin_api::barrier::{Frame, Scratch, noise_hash as h};
use kt_plugin_api::math::FrozenMath;

use crate::cfa::{self, IR};
use crate::{DYE_NIR, IR_FROM_RGB, IR_VIS_LEAK, Plan, fmt, pass};

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
/// The hash streams (each a different XOR of a seed).
const S_PRNU: u32 = 0x1111_1111;
const S_DSNU: u32 = 0x2222_2222;
const S_COLUMN: u32 = 0x3333_3333;
const S_READ: u32 = 0x5555_5555;
const S_ROW: u32 = 0x7777_7777;

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

/// The source's transfer undone (design step 1): 0 sRGB, 1 BT.709, 2 gamma 2.2, 3 linear.
#[inline(always)]
pub fn decode(c: u32, curve: u32) -> f32 {
    let v = c as f32 * INV_255;
    match curve {
        0 => {
            if v <= 0.04045 {
                v * 0.077_399_38
            } else {
                ((v + 0.055) * 0.947_867_3).fpowf(2.4)
            }
        }
        1 => {
            if v < 0.081 {
                v * 0.222_222_22
            } else {
                ((v + 0.099) * 0.909_918_1).fpowf(2.222_222_2)
            }
        }
        2 => v.fpowf(2.2),
        _ => v,
    }
}

/// sRGB decode of a byte (the default source).
#[inline(always)]
pub fn srgb_lin(c: u32) -> f32 {
    decode(c, 0)
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
fn t16(v: u32) -> u32 {
    texel(v >> 8, v & 0xFF, 0, 255)
}

/// The scene texel: R and G in 11 bits, B in 10, each `√(v/2)` of a linear value in [0, 2].
#[inline(always)]
fn pack_scene(rgb: [f32; 3]) -> u32 {
    let q = |v: f32, max: f32| ((v.clamp(0.0, 2.0) * 0.5).sqrt() * max + 0.5).floor() as u32;
    q(rgb[0], 2047.0) | q(rgb[1], 2047.0) << 11 | q(rgb[2], 1023.0) << 22
}

#[inline(always)]
fn unpack_scene(t: u32) -> [f32; 3] {
    let d = |q: u32, inv: f32| {
        let s = q as f32 * inv;
        s * s * 2.0
    };
    [d(t & 0x7FF, 4.885_197_8e-4), d(t >> 11 & 0x7FF, 4.885_197_8e-4), d(t >> 22, 9.775_171_1e-4)]
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

    /// The previous pass at FRAME pixel (x, y), clamped.
    #[inline(always)]
    fn at_frame(&self, x: i32, y: i32) -> u32 {
        let x = x.clamp(0, self.w as i32 - 1) as usize;
        let y = y.clamp(0, self.h as i32 - 1) as usize;
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

    #[inline(always)]
    fn src_byte(&self, x: i32, y: i32, c: usize) -> u32 {
        let x = x.clamp(0, self.w as i32 - 1) as u32;
        let y = y.clamp(0, self.h as i32 - 1) as u32;
        self.src[((y * self.w + x) * 4) as usize + c] as u32
    }

    /// The squared radius of a sensor pixel's centre over the squared half-diagonal.
    #[inline(always)]
    fn r2(&self, sx: i32, sy: i32) -> f32 {
        let (hw, hh) = (self.sw as f32 * 0.5, self.sh as f32 * 0.5);
        let cx = sx as f32 + 0.5 - hw;
        let cy = sy as f32 + 0.5 - hh;
        (cx * cx + cy * cy) / (hw * hw + hh * hh)
    }

    /// One channel of the unprocessed scene, bilinear at (px, py) in SENSOR pixel units.
    #[inline(always)]
    fn scene(&self, px: f32, py: f32, c: usize) -> f32 {
        let k = self.c.k as f32;
        let fx = px * k - 0.5;
        let fy = py * k - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let s = |x: i32, y: i32| unpack_scene(self.at_frame(x, y))[c];
        let (xa, ya) = (x0 as i32, y0 as i32);
        let top = s(xa, ya) + (s(xa + 1, ya) - s(xa, ya)) * tx;
        let bot = s(xa, ya + 1) + (s(xa + 1, ya + 1) - s(xa, ya + 1)) * tx;
        top + (bot - top) * ty
    }
}

/// The defocus disc's taps: the centre, eight at 0.6 r and eight at r, at multiples of 45°.
const DISC: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.707_106_78, 0.707_106_78),
    (0.0, 1.0),
    (-0.707_106_78, 0.707_106_78),
    (-1.0, 0.0),
    (-0.707_106_78, -0.707_106_78),
    (0.0, -1.0),
    (0.707_106_78, -0.707_106_78),
];

// ---- U: the unprocessing (frame pixels) ----------------------------------------------------------

#[inline(always)]
pub(crate) fn unprocess_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let lin = |x: i32, y: i32| -> [f32; 3] {
        [
            decode(cx.src_byte(x, y, 0), c.src_curve),
            decode(cx.src_byte(x, y, 1), c.src_curve),
            decode(cx.src_byte(x, y, 2), c.src_curve),
        ]
    };
    for (x, o) in out.iter_mut().enumerate() {
        let (xi, yi) = (x as i32, y as i32);
        let mut s = lin(xi, yi);
        if c.blur > 0.0 {
            for r in [0.6 * c.blur, c.blur] {
                for (dx, dy) in DISC {
                    let ox = (dx * r + 0.5).floor() as i32;
                    let oy = (dy * r + 0.5).floor() as i32;
                    let q = lin(xi + ox, yi + oy);
                    s = [s[0] + q[0], s[1] + q[1], s[2] + q[2]];
                }
            }
            let inv = 0.058_823_53;
            s = [s[0] * inv, s[1] * inv, s[2] * inv];
        }
        let m = &c.src_m;
        let r = (m[0] * s[0] + m[1] * s[1] + m[2] * s[2]) * c.src_wb_inv[0];
        let g = m[3] * s[0] + m[4] * s[1] + m[5] * s[2];
        let b = (m[6] * s[0] + m[7] * s[1] + m[8] * s[2]) * c.src_wb_inv[1];
        *o = pack_scene([r, g, b]);
    }
}

// ---- P0: the lens and the photons ----------------------------------------------------------------

/// The scene through the lens at a sensor pixel: distortion and CA per channel, the 4-spot OLPF,
/// cos⁴ vignetting and colour shading.
#[inline(always)]
fn optics(cx: &Ctx, sx: i32, sy: i32) -> [f32; 3] {
    let c = cx.c;
    let (px, py) = (sx as f32 + 0.5, sy as f32 + 0.5);
    if !c.optics {
        return [cx.scene(px, py, 0), cx.scene(px, py, 1), cx.scene(px, py, 2)];
    }
    let (hw, hh) = (cx.sw as f32 * 0.5, cx.sh as f32 * 0.5);
    let (dx, dy) = (px - hw, py - hh);
    let hd2 = hw * hw + hh * hh;
    let dist = 1.0 + c.k_lens * ((dx * dx + dy * dy) / hd2);
    let ca = c.ca / hd2.sqrt();
    let d = c.olpf * 0.5;
    let mut out = [0.0f32; 3];
    for (ch, o) in out.iter_mut().enumerate() {
        let s = dist * (1.0 + ca * (1.0 - ch as f32));
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
    let r2 = cx.r2(sx, sy);
    let t = 1.0 + c.vig_t2 * r2;
    let e = 1.0 / (t * t);
    [out[0] * e * (1.0 - c.cshade * r2), out[1] * e, out[2] * e * (1.0 + 0.5 * c.cshade * r2)]
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
pub(crate) fn optics_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let k = cx.c.k;
    let sy = (y / k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / k) as i32;
        let code = cfa::code(&cx.c.sensor_tab, cx.c.period, sx, sy);
        *o = t16(fix16(photons(cx.c, code, optics(cx, sx, sy))));
    }
}

// ---- P1a: the pixel — charge ----------------------------------------------------------------------

#[inline(always)]
pub(crate) fn pixel_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let w = cx.w as usize;
    let sy = y / c.k;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = x as u32 / c.k;
        let lam_s = v16(cx.inp[y as usize * w + x]) as f32 * INV_FIX * c.fw;
        let hf = hash2(sx, sy, c.seed_f);
        let n = if hf < c.dead_thr {
            0.0
        } else {
            let mut gpx = 1.0 + c.prnu * gauss(hash2(sx, sy, c.seed_f ^ S_PRNU));
            // Gr/Gb imbalance: a green pixel beside red differs from one beside blue
            let code = cfa::code(&c.sensor_tab, c.period, sx as i32, sy as i32);
            if code == cfa::G && c.gimb > 0.0 {
                let side = cfa::code(&c.sensor_tab, c.period, sx as i32 + 1, sy as i32);
                if side == cfa::R {
                    gpx *= 1.0 + c.gimb;
                } else if side == cfa::B {
                    gpx *= 1.0 - c.gimb;
                }
            }
            let mut dk = (c.dark + c.dsnu * gauss(hash2(sx, sy, c.seed_f ^ S_DSNU))).max(0.0);
            if hf - c.dead_thr < c.hot_thr {
                dk += c.fw * (0.2 + 0.8 * unit(h(hf)));
            }
            let lam = lam_s * gpx + dk;
            if c.noise { poisson(lam, hash2(sx, sy, c.seed_t)) } else { lam }
        };
        *o = t16(fix16(n * c.inv_fw));
    }
}

// ---- P1b: the readout — blooming, the well, row/column noise, read noise, gain, ADC ---------------

#[inline(always)]
pub(crate) fn readout_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let n_at = |sx: i32, sy: i32| v16(cx.at(sx, sy)) as f32 * INV_FIX * c.fw;
    let row =
        if c.row_e > 0.0 { c.row_e * gauss(hash2(0, sy as u32, c.seed_t ^ S_ROW)) } else { 0.0 };
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let mut n = n_at(sx, sy);
        if c.bloom > 0.0 {
            let spill = |q: f32| (q - c.fw).max(0.0);
            let s = spill(n_at(sx - 1, sy))
                + spill(n_at(sx + 1, sy))
                + spill(n_at(sx, sy - 1))
                + spill(n_at(sx, sy + 1));
            n += s * (c.bloom * 0.25);
        }
        n = n.min(c.fw);
        n += row;
        if c.col_e > 0.0 {
            n += c.col_e * gauss(hash2(sx as u32, 0, c.seed_f ^ S_COLUMN));
        }
        if c.noise {
            n += c.read * gauss(hash2(sx as u32, sy as u32, c.seed_t ^ S_READ));
        }
        let dn = (c.black as f32 + n * c.cg + 0.5).floor().clamp(0.0, c.adc_max as f32);
        *o = t16(dn as u32);
    }
}

// ---- P2a: the raw front — BLC, optical-black row/column correction, DPC, green equalisation ------

#[inline(always)]
pub(crate) fn raw_front_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let tab = &c.isp_tab;
    // the black-level-subtracted raw of a sensor pixel, with the optical-black correction of its row
    // and column
    let vb = |sx: i32, sy: i32| {
        let mut v = (v16(cx.at(sx, sy)) as f32 - c.b_isp) * c.inv_range;
        if c.row_corr != 0.0 {
            v -= c.row_corr * gauss(hash2(0, sy as u32, c.seed_t ^ S_ROW));
        }
        if c.col_corr != 0.0 {
            v -= c.col_corr * gauss(hash2(sx as u32, 0, c.seed_f ^ S_COLUMN));
        }
        v
    };
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
        if c.geq > 0.0 && code == cfa::G {
            let (mut s, mut n) = (0.0f32, 0.0f32);
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                if cfa::code(tab, c.period, sx + dx, sy + dy) == cfa::G {
                    let (nx, ny) = cx.wrap(sx + dx, sy + dy);
                    s += vb(nx, ny);
                    n += 1.0;
                }
            }
            if n > 0.0 {
                let d = s / n - v;
                if d.abs() < 0.05 {
                    v += c.geq * 0.5 * d;
                }
            }
        }
        *o = t16(fix16(v.max(0.0)));
    }
}

// ---- P2n: raw NR by the sensor's noise model -----------------------------------------------------

#[inline(always)]
fn vfix(cx: &Ctx, sx: i32, sy: i32) -> f32 {
    let (nx, ny) = cx.wrap(sx, sy);
    v16(cx.at(nx, ny)) as f32 * INV_FIX
}

#[inline(always)]
pub(crate) fn raw_nr_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let k2 = 4.0 * c.raw_nr * c.raw_nr;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let code = cfa::code(&c.isp_tab, c.period, sx, sy);
        let v0 = vfix(cx, sx, sy);
        // the noise the sensor model predicts at this level: shot (∝ signal) and read
        let h2 = k2 * (c.nr_a * v0.max(0.0) + c.nr_b) + 1.0e-12;
        let (mut s, mut sw) = (0.0f32, 0.0f32);
        for dy in -2..=2 {
            for dx in -2..=2 {
                if cfa::code(&c.isp_tab, c.period, sx + dx, sy + dy) != code {
                    continue;
                }
                let q = vfix(cx, sx + dx, sy + dy);
                let d = q - v0;
                let wt = 1.0 / (1.0 + d * d / h2);
                s += wt * q;
                sw += wt;
            }
        }
        *o = t16(fix16(s / sw));
    }
}

// ---- P2b: IR subtraction, lens shading (luma and colour), white balance --------------------------

#[inline(always)]
pub(crate) fn raw_correct_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let tab = &c.isp_tab;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let code = cfa::code(tab, c.period, sx, sy);
        let mut v = vfix(cx, sx, sy);
        if c.ir_sub > 0.0 && code != IR {
            let (mut s, mut n) = (0.0f32, 0.0f32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if cfa::code(tab, c.period, sx + dx, sy + dy) == IR {
                        s += vfix(cx, sx + dx, sy + dy);
                        n += 1.0;
                    }
                }
            }
            if n > 0.0 {
                v -= c.ir_sub * (DYE_NIR[code as usize] / DYE_NIR[IR as usize]) * (s / n);
            }
        }
        let r2 = cx.r2(sx, sy);
        if c.lsc_pow > 0.0 {
            v *= (1.0 + c.vig_t2 * r2).fpowf(c.lsc_pow);
        }
        if c.clsc > 0.0 {
            if code == cfa::R {
                v *= (1.0 - c.cshade * r2).max(0.05).fpowf(-c.clsc);
            } else if code == cfa::B {
                v *= (1.0 + 0.5 * c.cshade * r2).max(0.05).fpowf(-c.clsc);
            }
        }
        if code < 4 {
            v *= c.gains[code as usize];
        }
        *o = t16(fix16(v.max(0.0)));
    }
}

// ---- P3: demosaic, highlights, CCM, tone --------------------------------------------------------------------

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
pub(crate) fn demosaic_row(cx: &Ctx, y: u32, out: &mut [u32]) {
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
        // highlight handling: near the level where the first channel clips, the channels stop
        // tracking each other (a clipped channel tints the highlight); the ISP pulls them to luma
        if c.hl > 0.0 {
            let m = rgb[0].max(rgb[1]).max(rgb[2]);
            let t = ((m - 0.9 * c.hl_clip) / (0.1 * c.hl_clip)).clamp(0.0, 1.0) * c.hl;
            let yy = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
            for v in rgb.iter_mut() {
                *v += (yy - *v) * t;
            }
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

// ---- P3b: lens correction — dewarp and lateral CA correction -------------------------------------

/// The captured picture (RGB8 per sensor pixel) bilinear at (px, py) in sensor units, one channel.
#[inline(always)]
fn rgb_at(cx: &Ctx, px: f32, py: f32, ch: u32) -> f32 {
    let fx = px - 0.5;
    let fy = py - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let s = |x: i32, y: i32| (cx.at(x, y) >> (8 * ch) & 0xFF) as f32;
    let (xa, ya) = (x0 as i32, y0 as i32);
    let top = s(xa, ya) + (s(xa + 1, ya) - s(xa, ya)) * tx;
    let bot = s(xa, ya + 1) + (s(xa + 1, ya + 1) - s(xa, ya + 1)) * tx;
    top + (bot - top) * ty
}

#[inline(always)]
pub(crate) fn lens_correct_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let sy = (y / c.k) as i32;
    let (hw, hh) = (cx.sw as f32 * 0.5, cx.sh as f32 * 0.5);
    let hd2 = hw * hw + hh * hh;
    let ca = c.ca_corr / hd2.sqrt();
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let (dx, dy) = (sx as f32 + 0.5 - hw, sy as f32 + 0.5 - hh);
        let ro = ((dx * dx + dy * dy) / hd2).sqrt();
        let mut rgb = [0u32; 3];
        for (ch, v) in rgb.iter_mut().enumerate() {
            // the captured radius x with x(1 + k x²)(1 + ca_ch) = r_o, by Newton from r_o
            let t = ro / (1.0 + ca * (1.0 - ch as f32));
            let mut xr = t;
            for _ in 0..3 {
                let fx = xr * (1.0 + c.k_corr * xr * xr) - t;
                xr -= fx / (1.0 + 3.0 * c.k_corr * xr * xr);
            }
            let sc = if ro > 0.0 { xr / ro } else { 1.0 };
            *v = (rgb_at(cx, hw + dx * sc, hh + dy * sc, ch as u32) + 0.5).floor().clamp(0.0, 255.0)
                as u32;
        }
        *o = texel(rgb[0], rgb[1], rgb[2], cx.src_alpha(x as u32, y));
    }
}

// ---- P4: YUV, denoise, false colour and purple fringe, sharpening ---------------------------------

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
pub(crate) fn yuv_row(cx: &Ctx, y: u32, out: &mut [u32]) {
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
        // false-colour suppression: chroma at a luma edge is the demosaic's guess, not the scene's
        if c.fcs > 0.0 {
            let gx = (yuv(cx, sx + 1, sy)[0] - yuv(cx, sx - 1, sy)[0]).abs();
            let gy = (yuv(cx, sx, sy + 1)[0] - yuv(cx, sx, sy - 1)[0]).abs();
            let k = 1.0 - c.fcs * (gx.max(gy) * 4.0).clamp(0.0, 1.0);
            u *= k;
            v *= k;
        }
        // purple fringe: blue-and-red above luma next to a highlight (blooming, axial CA)
        if c.purple > 0.0 && u > 0.0 && v > 0.0 {
            let mut my = 0.0f32;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    my = my.max(yuv(cx, sx + dx, sy + dy)[0]);
                }
            }
            let k = 1.0
                - c.purple * ((my - 0.9) * 10.0).clamp(0.0, 1.0) * (u.min(v) * 8.0).clamp(0.0, 1.0);
            u *= k;
            v *= k;
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
pub(crate) fn chroma_row(cx: &Ctx, y: u32, out: &mut [u32]) {
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

// ---- PV: the view of a stage --------------------------------------------------------------------

/// A mosaic value shown in its filter's colour (W grey, IR magenta), sRGB-coded for the screen.
#[inline(always)]
fn tint(code: u32, v: f32) -> [u32; 3] {
    let e = byte(oetf(v, 1));
    match code {
        0 => [e, 0, 0],
        1 => [0, e, 0],
        2 => [0, 0, e],
        3 => [e, e, e],
        _ => [e, 0, e],
    }
}

#[inline(always)]
pub(crate) fn view_row(cx: &Ctx, y: u32, out: &mut [u32]) {
    let c = cx.c;
    let f = c.view_fmt();
    let tab = if f == fmt::NORM { &c.isp_tab } else { &c.sensor_tab };
    let sy = (y / c.k) as i32;
    for (x, o) in out.iter_mut().enumerate() {
        let sx = (x as u32 / c.k) as i32;
        let a = cx.src_alpha(x as u32, y);
        let code = cfa::code(tab, c.period, sx, sy);
        let rgb = match f {
            fmt::SCENE => {
                let s = unpack_scene(cx.at_frame(x as i32, y as i32));
                [byte(oetf(s[0], 1)), byte(oetf(s[1], 1)), byte(oetf(s[2], 1))]
            }
            fmt::WELL | fmt::NORM => tint(code, v16(cx.at(sx, sy)) as f32 * INV_FIX),
            fmt::DN => {
                let dn = v16(cx.at(sx, sy)) as f32;
                tint(code, (dn - c.black as f32) / (c.adc_max - c.black) as f32)
            }
            fmt::LUMA => {
                let yy = cx.at(sx, sy) & 0xFF;
                [yy, yy, yy]
            }
            _ => {
                let t = cx.at(sx, sy);
                let u = (t >> 8 & 0xFF) as f32 * INV_255 - 0.5;
                let v = (t >> 16 & 0xFF) as f32 * INV_255 - 0.5;
                let [r, g, b] = rgb_of(&c.dec_same, 0.5, u, v);
                [byte(r), byte(g), byte(b)]
            }
        };
        *o = texel(rgb[0], rgb[1], rgb[2], a);
    }
}

// ---- the driver ---------------------------------------------------------------------------------

pub(crate) type Row = fn(&Ctx, u32, &mut [u32]);

/// The row functions by pass index (`crate::pass`).
pub(crate) const ROWS: [Row; 12] = [
    unprocess_row,
    optics_row,
    pixel_row,
    readout_row,
    raw_front_row,
    raw_nr_row,
    raw_correct_row,
    demosaic_row,
    lens_correct_row,
    yuv_row,
    chroma_row,
    view_row,
];

thread_local! {
    /// The two ping-pong planes, reused per thread (invariant 12: no allocation per frame once warm).
    static PLANES: RefCell<(Vec<u32>, Vec<u32>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// One pass over all rows, split across scoped threads (rows are independent: no effect on bytes).
fn run_pass(cx: &Ctx, out: &mut [u32], f: Row) {
    let w = cx.w as usize;
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

fn row_fn(p: usize, simd: bool) -> Row {
    #[cfg(target_arch = "x86_64")]
    if simd {
        return crate::simd::TWINS[p];
    }
    let _ = simd;
    ROWS[p]
}

/// The whole camera on the CPU: the passes of `Plan::passes`, the frame handed back with the
/// output bytes.
pub fn run(plan: &Plan, mut f: Frame, simd: bool, _scratch: &mut dyn Scratch) -> Frame {
    let (w, h) = (f.w, f.h);
    if w == 0 || h == 0 {
        return f;
    }
    let n = (w * h) as usize;
    let k = plan.k;
    let (sw, sh) = (w.div_ceil(k) as i32, h.div_ceil(k) as i32);
    let passes = plan.passes();
    PLANES.with(|planes| {
        let mut planes = planes.borrow_mut();
        let (a, b) = &mut *planes;
        a.resize(n, 0);
        b.resize(n, 0);
        let src = &f.rgba;
        // the passes ping-pong: the first reads only the source
        let mut in_a = false;
        for (i, p) in passes.iter().enumerate() {
            let (inp, out): (&[u32], &mut [u32]) = if i == 0 {
                (&[], &mut a[..])
            } else if in_a {
                (&a[..], &mut b[..])
            } else {
                (&b[..], &mut a[..])
            };
            let cx = Ctx { c: plan, w, h, sw, sh, src, inp };
            run_pass(&cx, out, row_fn(*p, simd));
            in_a = i == 0 || !in_a;
        }
        let last: &[u32] = if in_a { a } else { b };
        for (px, t) in f.rgba.chunks_exact_mut(4).zip(last) {
            px.copy_from_slice(&t.to_le_bytes());
        }
    });
    f
}

/// The plane after the pass `upto` of the plan's list (for tests that read the raw, t1212). Allocates.
pub fn plane_after(plan: &Plan, f: &Frame, upto: usize) -> Vec<u32> {
    let (w, h) = (f.w, f.h);
    let n = (w * h) as usize;
    let k = plan.k;
    let (sw, sh) = (w.div_ceil(k) as i32, h.div_ceil(k) as i32);
    let mut prev = vec![0u32; n];
    for p in plan.passes() {
        if p == pass::VIEW {
            break;
        }
        let mut out = vec![0u32; n];
        let cx = Ctx { c: plan, w, h, sw, sh, src: &f.rgba, inp: &prev };
        run_pass(&cx, &mut out, ROWS[p]);
        prev = out;
        if p == upto {
            break;
        }
    }
    prev
}
