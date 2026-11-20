//! The exact RGB → YUV conversion of an export (export-host-copy D2, found-defects 741).
//!
//! One integer formula, the same on the composing device (kt-gpu's pass reads these coefficients) and
//! on the processor (this reference, every path without the pass): an export's YUV is a function of its
//! RGBA alone, on every device and machine (invariant 11).
//!
//! - The matrix (BT.601 / BT.709) and the range (limited 16–235/240, full) give Q15 coefficients,
//!   computed here in integers from the rational Kr, Kb — no float, no libm.
//!   `Y = (cy·RGB + off_y + 2^14) >> 15`; chroma is kept at Q15 per pixel and rounded only after the
//!   filter.
//! - Chroma sited LEFT (co-sited horizontally, centred vertically — what a player assumes of an
//!   untagged H.264/HEVC stream): 4:2:0 `C = ([1 2 1] at the even column) · ([1 1] down) / 8`, 4:2:2 the
//!   horizontal filter only (/4); an edge column or row repeats.
//! - 10-bit: the same at the 10-bit offsets and scales; P010 keeps the sample in the high ten bits.
//! - The planes are tightly packed, in the order libav's `AVFrame` takes them; 16-bit samples are
//!   little-endian.

/// The encoder input formats (`kt_ffmpeg::encoders::PixFmt`, a closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YuvPix {
    Yuv420p,
    Nv12,
    Yuv420p10,
    P010,
    Yuv422p10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YuvMatrix {
    Bt601,
    Bt709,
}

/// What a readback of an export frame is: the encoder's planes in this layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct YuvLayout {
    pub pix: YuvPix,
    pub matrix: YuvMatrix,
    /// Full range (0–max) instead of limited (16–235/240 at 8 bits).
    pub full: bool,
}

/// One plane of a frame: where it starts in the packed bytes, its row's bytes and its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plane {
    pub offset: usize,
    pub row_bytes: usize,
    pub rows: usize,
}

/// The integer coefficients a conversion runs on (the shader takes the same numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coeffs {
    /// Q15 weights of R, G, B for Y, Cb, Cr.
    pub y: [i32; 3],
    pub cb: [i32; 3],
    pub cr: [i32; 3],
    /// Q15 offsets of Y and of chroma.
    pub off_y: i32,
    pub off_c: i32,
    /// The largest sample (255 or 1023).
    pub max: i32,
}

impl YuvPix {
    pub fn ten_bit(self) -> bool {
        matches!(self, YuvPix::Yuv420p10 | YuvPix::P010 | YuvPix::Yuv422p10)
    }

    /// Chroma rows are halved (4:2:0).
    pub fn half_rows(self) -> bool {
        !matches!(self, YuvPix::Yuv422p10)
    }

    /// U and V interleaved in one plane.
    pub fn semi_planar(self) -> bool {
        matches!(self, YuvPix::Nv12 | YuvPix::P010)
    }

    /// The number this layout has in kt-gpu's pass (`yuv.wgsl`).
    pub fn code(self) -> u32 {
        match self {
            YuvPix::Yuv420p => 0,
            YuvPix::Nv12 => 1,
            YuvPix::Yuv420p10 => 2,
            YuvPix::P010 => 3,
            YuvPix::Yuv422p10 => 4,
        }
    }
}

/// Rounded `n / d`, halves away from zero, in integers.
fn div_round(n: i64, d: i64) -> i64 {
    if n >= 0 { (n + d / 2) / d } else { -((-n + d / 2) / d) }
}

impl YuvLayout {
    /// The chroma plane's size for a `w`×`h` frame.
    pub fn chroma_size(&self, w: u32, h: u32) -> (usize, usize) {
        let cw = (w as usize).div_ceil(2);
        let ch = if self.pix.half_rows() { (h as usize).div_ceil(2) } else { h as usize };
        (cw, ch)
    }

    /// The planes of a `w`×`h` frame, packed: Y, then U and V (or UV interleaved).
    pub fn planes(&self, w: u32, h: u32) -> Vec<Plane> {
        let s = if self.pix.ten_bit() { 2 } else { 1 };
        let (w, h) = (w as usize, h as usize);
        let (cw, ch) = self.chroma_size(w as u32, h as u32);
        let y = Plane { offset: 0, row_bytes: w * s, rows: h };
        let after_y = w * s * h;
        if self.pix.semi_planar() {
            vec![y, Plane { offset: after_y, row_bytes: cw * 2 * s, rows: ch }]
        } else {
            let c = cw * s * ch;
            vec![
                y,
                Plane { offset: after_y, row_bytes: cw * s, rows: ch },
                Plane { offset: after_y + c, row_bytes: cw * s, rows: ch },
            ]
        }
    }

    /// The packed frame's bytes.
    pub fn frame_bytes(&self, w: u32, h: u32) -> usize {
        self.planes(w, h).iter().map(|p| p.row_bytes * p.rows).sum()
    }

    /// The Q15 coefficients of this matrix, range and depth, from the rational Kr, Kb in integers.
    pub fn coeffs(&self) -> Coeffs {
        // Kr, Kb over a common denominator
        let (kr, kb, d): (i64, i64, i64) = match self.matrix {
            YuvMatrix::Bt601 => (299, 114, 1000),
            YuvMatrix::Bt709 => (2126, 722, 10000),
        };
        let kg = d - kr - kb;
        let bits = if self.pix.ten_bit() { 10 } else { 8 };
        let m = 1i64 << (bits - 8);
        let max = (1i64 << bits) - 1;
        // Y = sy·E + oy, C = sc·E' + oc (E in 0..1, E' in -0.5..0.5); RGB arrives in 0..255
        let (sy, oy, sc) = if self.full { (max, 0, max) } else { (219 * m, 16 * m, 224 * m) };
        let oc = 1i64 << (bits - 1);
        let q = 1i64 << 15;
        let yr = div_round(sy * kr * q, d * 255);
        let yb = div_round(sy * kb * q, d * 255);
        // the three weights sum to sy/255 in Q15, so white is exactly the top of the range
        let yg = div_round(sy * q, 255) - yr - yb;
        // Cb = sc·(B − Ey) / (2(1 − Kb)), Cr = sc·(R − Ey) / (2(1 − Kr))
        let cbr = div_round(-sc * kr * q, 2 * (d - kb) * 255);
        let cbg = div_round(-sc * kg * q, 2 * (d - kb) * 255);
        let crb = div_round(-sc * kb * q, 2 * (d - kr) * 255);
        let crg = div_round(-sc * kg * q, 2 * (d - kr) * 255);
        // the weights of a chroma sum to zero, so every grey is exactly the middle
        let cbb = -cbr - cbg;
        let crr = -crg - crb;
        Coeffs {
            y: [yr as i32, yg as i32, yb as i32],
            cb: [cbr as i32, cbg as i32, cbb as i32],
            cr: [crr as i32, crg as i32, crb as i32],
            off_y: (oy * q) as i32,
            off_c: (oc * q) as i32,
            max: max as i32,
        }
    }
}

/// Convert a straight RGBA frame (`w`×`h`, rows of `w·4` bytes) into the layout's packed planes.
pub fn convert(layout: &YuvLayout, rgba: &[u8], w: u32, h: u32, out: &mut Vec<u8>) {
    let (wu, hu) = (w as usize, h as usize);
    assert!(rgba.len() >= wu * hu * 4, "an RGBA frame of {w}×{h} is {} bytes", rgba.len());
    let k = layout.coeffs();
    let planes = layout.planes(w, h);
    out.clear();
    out.resize(layout.frame_bytes(w, h), 0);
    let px = |x: usize, y: usize| {
        let i = (y.min(hu - 1) * wu + x.min(wu - 1)) * 4;
        [rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32]
    };
    let dot = |c: &[i32; 3], p: [i32; 3]| c[0] * p[0] + c[1] * p[1] + c[2] * p[2];
    let ten = layout.pix.ten_bit();
    let shift = if layout.pix == YuvPix::P010 { 6 } else { 0 };
    let put = |out: &mut [u8], at: usize, v: i32| {
        let v = v.clamp(0, k.max) as u16;
        if ten {
            out[at..at + 2].copy_from_slice(&(v << shift).to_le_bytes());
        } else {
            out[at] = v as u8;
        }
    };
    let s = if ten { 2 } else { 1 };
    for y in 0..hu {
        for x in 0..wu {
            let v = (dot(&k.y, px(x, y)) + k.off_y + (1 << 14)) >> 15;
            put(out, planes[0].offset + y * planes[0].row_bytes + x * s, v);
        }
    }
    let (cw, ch) = layout.chroma_size(w, h);
    let half_rows = layout.pix.half_rows();
    // a chroma sample at Q15 per pixel, filtered: [1 2 1] across, and [1 1] down for 4:2:0
    let chroma = |c: &[i32; 3], off: i32, cx: usize, cy: usize| -> i32 {
        let x0 = 2 * cx;
        let rows: &[usize] = if half_rows { &[2 * cy, 2 * cy + 1] } else { &[cy] };
        let mut sum = 0i32;
        for &ry in rows {
            let at = |x: usize| dot(c, px(x, ry)) + off;
            sum += at(x0.saturating_sub(1)) + 2 * at(x0) + at(x0 + 1);
        }
        let bits = if half_rows { 15 + 3 } else { 15 + 2 };
        (sum + (1 << (bits - 1))) >> bits
    };
    for cy in 0..ch {
        for cx in 0..cw {
            let u = chroma(&k.cb, k.off_c, cx, cy);
            let v = chroma(&k.cr, k.off_c, cx, cy);
            if layout.pix.semi_planar() {
                let at = planes[1].offset + cy * planes[1].row_bytes + cx * 2 * s;
                put(out, at, u);
                put(out, at + s, v);
            } else {
                put(out, planes[1].offset + cy * planes[1].row_bytes + cx * s, u);
                put(out, planes[2].offset + cy * planes[2].row_bytes + cx * s, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [YuvPix; 5] = [YuvPix::Yuv420p, YuvPix::Nv12, YuvPix::Yuv420p10, YuvPix::P010, YuvPix::Yuv422p10];

    fn solid(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    /// A sample of a packed frame: plane `p`, column `x` of its row `y` (`k`-th of a pair in a
    /// semi-planar plane).
    fn sample(l: &YuvLayout, f: &[u8], w: u32, h: u32, p: usize, x: usize, y: usize, k: usize) -> i32 {
        let pl = l.planes(w, h)[p];
        let s = if l.pix.ten_bit() { 2 } else { 1 };
        let per = if p == 1 && l.pix.semi_planar() { 2 } else { 1 };
        let at = pl.offset + y * pl.row_bytes + (x * per + k) * s;
        if s == 2 {
            let v = u16::from_le_bytes([f[at], f[at + 1]]);
            (if l.pix == YuvPix::P010 { v >> 6 } else { v }) as i32
        } else {
            f[at] as i32
        }
    }

    /// (Y, Cb, Cr) of a solid frame.
    fn ycc(l: &YuvLayout, rgb: [u8; 3]) -> (i32, i32, i32) {
        let mut f = Vec::new();
        convert(l, &solid(4, 4, rgb), 4, 4, &mut f);
        let (u, v) = if l.pix.semi_planar() { ((1, 0), (1, 1)) } else { ((1, 0), (2, 0)) };
        (sample(l, &f, 4, 4, 0, 1, 1, 0), sample(l, &f, 4, 4, u.0, 1, 1, u.1), sample(l, &f, 4, 4, v.0, 1, 1, v.1))
    }

    /// The formula by hand, in exact rationals, rounded once: what the Q15 integers must give.
    fn by_hand(l: &YuvLayout, rgb: [u8; 3]) -> (i32, i32, i32) {
        let (kr, kb) = match l.matrix {
            YuvMatrix::Bt601 => (0.299, 0.114),
            YuvMatrix::Bt709 => (0.2126, 0.0722),
        };
        let bits = if l.pix.ten_bit() { 10 } else { 8 };
        let m = (1 << (bits - 8)) as f64;
        let max = ((1 << bits) - 1) as f64;
        let (sy, oy, sc) = if l.full { (max, 0.0, max) } else { (219.0 * m, 16.0 * m, 224.0 * m) };
        let oc = (1 << (bits - 1)) as f64;
        let [r, g, b] = rgb.map(|v| v as f64 / 255.0);
        let ey = kr * r + (1.0 - kr - kb) * g + kb * b;
        let y = sy * ey + oy;
        let cb = sc * (b - ey) / (2.0 * (1.0 - kb)) + oc;
        let cr = sc * (r - ey) / (2.0 * (1.0 - kr)) + oc;
        let c = |v: f64| (v + 0.5).floor().clamp(0.0, max) as i32;
        (c(y), c(cb), c(cr))
    }

    /// Test t3312 (export-host-copy D2): black, white, the primaries and secondaries give the values
    /// computed by hand, for both matrices, both ranges, 8 and 10 bit, 4:2:0 and 4:2:2, every layout;
    /// a one-pixel stripe gives the left-sited chroma; odd sizes repeat the edge.
    #[test]
    fn t3312_the_formula_s_values() {
        let colours = [
            [0, 0, 0],
            [255, 255, 255],
            [128, 128, 128],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 255, 0],
            [0, 255, 255],
            [255, 0, 255],
        ];
        for pix in ALL {
            for matrix in [YuvMatrix::Bt601, YuvMatrix::Bt709] {
                for full in [false, true] {
                    let l = YuvLayout { pix, matrix, full };
                    for c in colours {
                        let got = ycc(&l, c);
                        let want = by_hand(&l, c);
                        let off = |a: i32, b: i32| (a - b).abs();
                        assert!(
                            off(got.0, want.0) <= 0 && off(got.1, want.1) <= 1 && off(got.2, want.2) <= 1,
                            "{l:?} {c:?}: {got:?} against {want:?} by hand"
                        );
                    }
                    // the ends of the range and a neutral grey are exact
                    let (y0, cb0, cr0) = ycc(&l, [0, 0, 0]);
                    let (y1, cb1, cr1) = ycc(&l, [255, 255, 255]);
                    let m = if pix.ten_bit() { 4 } else { 1 };
                    let mid = if pix.ten_bit() { 512 } else { 128 };
                    if full {
                        assert_eq!((y0, y1), (0, if pix.ten_bit() { 1023 } else { 255 }), "{l:?}");
                    } else {
                        assert_eq!((y0, y1), (16 * m, 235 * m), "{l:?}");
                    }
                    assert_eq!((cb0, cr0, cb1, cr1), (mid, mid, mid, mid), "{l:?}: grey has no chroma");
                }
            }
        }
        // the left siting: a stripe of one red column at x = 1 in black (8-bit, BT.601 limited)
        let l = YuvLayout { pix: YuvPix::Yuv420p, matrix: YuvMatrix::Bt601, full: false };
        let (w, h) = (6u32, 2u32);
        let mut rgba = solid(w, h, [0, 0, 0]);
        for y in 0..h as usize {
            let i = (y * w as usize + 1) * 4;
            rgba[i..i + 3].copy_from_slice(&[255, 0, 0]);
        }
        let mut f = Vec::new();
        convert(&l, &rgba, w, h, &mut f);
        // red's Cr at Q15 weighted [1 2 1]: column 0 sees it once (x0+1), column 2 once (x0−1), 4 none
        let cr = |x: usize| sample(&l, &f, w, h, 2, x, 0, 0);
        let k = l.coeffs();
        let red = k.cr[0] * 255 + k.off_c;
        let black = k.off_c;
        let want = |n: i32| (n * red + (8 - n) * black + (1 << 17)) >> 18;
        assert_eq!((cr(0), cr(1), cr(2)), (want(2), want(2), black >> 15), "left-sited chroma: {f:?}");
        // odd sizes: the chroma planes take the ceiling, the edge repeats
        let l = YuvLayout { pix: YuvPix::Nv12, matrix: YuvMatrix::Bt709, full: false };
        let (w, h) = (5u32, 3u32);
        let rgba = solid(w, h, [10, 200, 30]);
        convert(&l, &rgba, w, h, &mut f);
        assert_eq!(f.len(), 5 * 3 + 3 * 2 * 2, "Y 5×3, UV 3×2 pairs");
        let corner = (sample(&l, &f, w, h, 1, 2, 1, 0), sample(&l, &f, w, h, 1, 2, 1, 1));
        let inner = (sample(&l, &f, w, h, 1, 0, 0, 0), sample(&l, &f, w, h, 1, 0, 0, 1));
        assert_eq!(corner, inner, "a solid frame's edge chroma equals the inside's");
    }
}
