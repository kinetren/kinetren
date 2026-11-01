//! The block codec (datamosh-codec-path, design D7): Y'CbCr 4:2:0 planes, block-matching motion
//! estimation, a closed prediction loop, an 8×8 DCT residual quantised by QP. No entropy coding,
//! no B-frames, integer-pel vectors — the MECHANISM of an inter-frame codec, not a standard's syntax.
//!
//! Determinism (invariant 11): integer arithmetic, plus IEEE f64 in the DCT with no fused ops and
//! no platform libm — the basis comes from the frozen `fcos`.

use kt_plugin_api::math::FrozenMath;
use std::sync::OnceLock;

/// Three 8-bit planes; `w`/`h` are the PADDED luma size (multiples of 16), chroma is half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planes {
    pub w: usize,
    pub h: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

/// The residual in the spatial domain (after the inverse DCT), per plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residual {
    pub y: Vec<i16>,
    pub cb: Vec<i16>,
    pub cr: Vec<i16>,
}

/// One coded frame: intra (predicted from flat 128) or inter (vectors per block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub intra: bool,
    /// Per luma block of `Cfg::bs`, row-major; (0,0) for an intra packet.
    pub mv: Vec<(i16, i16)>,
    pub res: Residual,
}

/// The encoder's output at one position: its own reconstruction (the closed loop's reference)
/// and the packet the decoder receives.
#[derive(Debug, Clone)]
pub struct Clean {
    pub recon: Planes,
    pub pkt: Packet,
}

/// The encoder's configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cfg {
    /// The luma block size of motion compensation (16 or 8).
    pub bs: usize,
    /// The search range, integer pels.
    pub search: i32,
    /// The quantiser parameter 0..51 (step `2^((qp − 4)/6)`).
    pub qp: f64,
}

impl Cfg {
    pub fn qstep(&self) -> f64 {
        2f64.fpowf((self.qp - 4.0) / 6.0)
    }
}

pub fn pad16(v: u32) -> usize {
    (v as usize).div_ceil(16) * 16
}

impl Planes {
    /// Flat grey (Y'CbCr 128, 128, 128): the reference a decoder starts from with no picture.
    pub fn grey(w: usize, h: usize) -> Planes {
        Planes { w, h, y: vec![128; w * h], cb: vec![128; w * h / 4], cr: vec![128; w * h / 4] }
    }

    /// RGBA `sw`×`sh` into padded planes `w`×`h` (edge replication). BT.601 full range, integer;
    /// chroma averaged over 2×2.
    pub fn from_rgba(rgba: &[u8], sw: u32, sh: u32, w: usize, h: usize) -> Planes {
        let (sw, sh) = (sw.max(1) as usize, sh.max(1) as usize);
        let mut y = vec![0u8; w * h];
        let mut cbf = vec![0i32; w * h];
        let mut crf = vec![0i32; w * h];
        for yy in 0..h {
            let sy = yy.min(sh - 1);
            for xx in 0..w {
                let sx = xx.min(sw - 1);
                let i = (sy * sw + sx) * 4;
                let (r, g, b) = (rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32);
                y[yy * w + xx] = ((77 * r + 150 * g + 29 * b + 128) >> 8).clamp(0, 255) as u8;
                cbf[yy * w + xx] = ((-43 * r - 85 * g + 128 * b + 128) >> 8) + 128;
                crf[yy * w + xx] = ((128 * r - 107 * g - 21 * b + 128) >> 8) + 128;
            }
        }
        let (cw, ch) = (w / 2, h / 2);
        let mut cb = vec![0u8; cw * ch];
        let mut cr = vec![0u8; cw * ch];
        for cy in 0..ch {
            for cx in 0..cw {
                let i = cy * 2 * w + cx * 2;
                let avg = |p: &[i32]| ((p[i] + p[i + 1] + p[i + w] + p[i + w + 1] + 2) >> 2).clamp(0, 255) as u8;
                cb[cy * cw + cx] = avg(&cbf);
                cr[cy * cw + cx] = avg(&crf);
            }
        }
        Planes { w, h, y, cb, cr }
    }

    /// Back to opaque RGBA, cropped to `ow`×`oh`; chroma by nearest neighbour.
    pub fn to_rgba(&self, ow: u32, oh: u32, out: &mut Vec<u8>) {
        let (ow, oh) = ((ow as usize).min(self.w), (oh as usize).min(self.h));
        out.clear();
        out.resize(ow * oh * 4, 255);
        let cw = self.w / 2;
        for yy in 0..oh {
            for xx in 0..ow {
                let l = self.y[yy * self.w + xx] as i32;
                let ci = (yy / 2) * cw + xx / 2;
                let cb = self.cb[ci] as i32 - 128;
                let cr = self.cr[ci] as i32 - 128;
                let o = (yy * ow + xx) * 4;
                out[o] = (l + ((359 * cr + 128) >> 8)).clamp(0, 255) as u8;
                out[o + 1] = (l - ((88 * cb + 183 * cr + 128) >> 8)).clamp(0, 255) as u8;
                out[o + 2] = (l + ((454 * cb + 128) >> 8)).clamp(0, 255) as u8;
            }
        }
    }

    pub fn bytes(&self) -> usize {
        self.y.len() + self.cb.len() + self.cr.len()
    }
}

// ---------------------------------------------------------------------------------------------
// the DCT
// ---------------------------------------------------------------------------------------------

/// The orthonormal 8×8 DCT-II basis `C[u][x]`.
fn basis() -> &'static [[f64; 8]; 8] {
    static B: OnceLock<[[f64; 8]; 8]> = OnceLock::new();
    B.get_or_init(|| {
        let mut c = [[0.0; 8]; 8];
        for (u, row) in c.iter_mut().enumerate() {
            let k = if u == 0 { 0.353_553_390_593_273_73 } else { 0.5 };
            for (x, v) in row.iter_mut().enumerate() {
                *v = k * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).fcos();
            }
        }
        c
    })
}

fn round_away(v: f64) -> f64 {
    if v >= 0.0 { (v + 0.5).floor() } else { -((-v + 0.5).floor()) }
}

/// Quantise and reconstruct one 8×8 block of residual in place (forward DCT, a uniform step with
/// rounding half away from zero, inverse DCT rounded to an integer).
fn quant_block(blk: &mut [i32; 64], q: f64) {
    let c = basis();
    let mut t = [0.0f64; 64];
    // rows: t[y][u] = Σx C[u][x] · b[y][x]
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += c[u][x] * blk[y * 8 + x] as f64;
            }
            t[y * 8 + u] = s;
        }
    }
    // columns: F[v][u] = Σy C[v][y] · t[y][u], quantised
    let mut f = [0.0f64; 64];
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += c[v][y] * t[y * 8 + u];
            }
            f[v * 8 + u] = round_away(s / q) * q;
        }
    }
    // inverse: columns then rows
    for u in 0..8 {
        for y in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += c[v][y] * f[v * 8 + u];
            }
            t[y * 8 + u] = s;
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += c[u][x] * t[y * 8 + u];
            }
            blk[y * 8 + x] = round_away(s) as i32;
        }
    }
}

/// `src − pred`, quantised per 8×8 block; returns the reconstructed residual.
fn code_residual(src: &[u8], pred: &[u8], w: usize, h: usize, q: f64) -> Vec<i16> {
    let mut out = vec![0i16; w * h];
    let mut blk = [0i32; 64];
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            for y in 0..8 {
                for x in 0..8 {
                    let i = (by + y) * w + bx + x;
                    blk[y * 8 + x] = src[i] as i32 - pred[i] as i32;
                }
            }
            quant_block(&mut blk, q);
            for y in 0..8 {
                for x in 0..8 {
                    out[(by + y) * w + bx + x] = blk[y * 8 + x].clamp(-512, 512) as i16;
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// motion
// ---------------------------------------------------------------------------------------------

/// Block-copy prediction of one plane: the block at (bx, by) of size `bs` reads the reference at
/// `+ (vx, vy)` with clamped coordinates (unrestricted vectors).
#[allow(clippy::too_many_arguments)]
fn mc_block(rf: &[u8], w: usize, h: usize, bx: usize, by: usize, bs: usize, vx: i32, vy: i32, out: &mut [u8]) {
    for y in 0..bs {
        let sy = (by as i32 + y as i32 + vy).clamp(0, h as i32 - 1) as usize;
        for x in 0..bs {
            let sx = (bx as i32 + x as i32 + vx).clamp(0, w as i32 - 1) as usize;
            out[(by + y) * w + bx + x] = rf[sy * w + sx];
        }
    }
}

/// The prediction of all three planes from `rf` by the vector field (one vector per luma block of
/// `bs`; the chroma block of `bs/2` takes the vector halved, floored).
pub fn predict(rf: &Planes, mv: &[(i16, i16)], bs: usize) -> Planes {
    let (w, h) = (rf.w, rf.h);
    let mut p = Planes::grey(w, h);
    let bw = w / bs;
    for (i, &(vx, vy)) in mv.iter().enumerate() {
        let (bx, by) = ((i % bw) * bs, (i / bw) * bs);
        let (vx, vy) = (vx as i32, vy as i32);
        mc_block(&rf.y, w, h, bx, by, bs, vx, vy, &mut p.y);
        let (cs, cw, chh) = (bs / 2, w / 2, h / 2);
        mc_block(&rf.cb, cw, chh, bx / 2, by / 2, cs, vx >> 1, vy >> 1, &mut p.cb);
        mc_block(&rf.cr, cw, chh, bx / 2, by / 2, cs, vx >> 1, vy >> 1, &mut p.cr);
    }
    p
}

#[allow(clippy::too_many_arguments)]
fn sad(cur: &[u8], rf: &[u8], w: usize, h: usize, bx: usize, by: usize, bs: usize, vx: i32, vy: i32) -> u32 {
    let mut s = 0u32;
    for y in 0..bs {
        let sy = (by as i32 + y as i32 + vy).clamp(0, h as i32 - 1) as usize;
        let row = (by + y) * w;
        for x in 0..bs {
            let sx = (bx as i32 + x as i32 + vx).clamp(0, w as i32 - 1) as usize;
            s += (cur[row + bx + x] as i32 - rf[sy * w + sx] as i32).unsigned_abs();
        }
    }
    s
}

/// The cost order: SAD, then the shorter vector, then (vy, vx) — total, so the search is
/// deterministic whatever the visiting order.
fn cost(s: u32, v: (i32, i32)) -> (u32, i32, i32, i32) {
    (s, v.0.abs() + v.1.abs(), v.1, v.0)
}

/// Block-matching motion estimation of `cur` against `rf` (luma): candidates (0,0) and the left
/// and top neighbours' vectors, a diamond of step 2 to convergence, a ±1 refinement.
pub fn estimate(cur: &Planes, rf: &Planes, cfg: &Cfg) -> Vec<(i16, i16)> {
    let (w, h, bs, r) = (cur.w, cur.h, cfg.bs, cfg.search.max(0));
    let (bw, bh) = (w / bs, h / bs);
    let mut mv = vec![(0i16, 0i16); bw * bh];
    let clampv = |v: (i32, i32)| (v.0.clamp(-r, r), v.1.clamp(-r, r));
    for byi in 0..bh {
        for bxi in 0..bw {
            let (bx, by) = (bxi * bs, byi * bs);
            let eval = |v: (i32, i32)| cost(sad(&cur.y, &rf.y, w, h, bx, by, bs, v.0, v.1), v);
            let mut best = (0, 0);
            let mut bc = eval(best);
            let mut cands = Vec::with_capacity(2);
            if bxi > 0 {
                let l = mv[byi * bw + bxi - 1];
                cands.push((l.0 as i32, l.1 as i32));
            }
            if byi > 0 {
                let t = mv[(byi - 1) * bw + bxi];
                cands.push((t.0 as i32, t.1 as i32));
            }
            for c in cands {
                let c = clampv(c);
                let cc = eval(c);
                if cc < bc {
                    (best, bc) = (c, cc);
                }
            }
            for step in [2, 1] {
                for _ in 0..16 {
                    let mut moved = false;
                    let pts: &[(i32, i32)] = if step == 2 {
                        &[(2, 0), (-2, 0), (0, 2), (0, -2)]
                    } else {
                        &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)]
                    };
                    let base = best;
                    for d in pts {
                        let c = clampv((base.0 + d.0, base.1 + d.1));
                        let cc = eval(c);
                        if cc < bc {
                            (best, bc) = (c, cc);
                            moved = true;
                        }
                    }
                    if !moved || step == 1 {
                        break;
                    }
                }
            }
            mv[byi * bw + bxi] = (best.0 as i16, best.1 as i16);
        }
    }
    mv
}

// ---------------------------------------------------------------------------------------------
// encode / decode
// ---------------------------------------------------------------------------------------------

fn add_clamp(pred: &[u8], res: &[i16], gain_q8: i32, out: &mut [u8]) {
    for ((o, &p), &r) in out.iter_mut().zip(pred).zip(res) {
        let r = if gain_q8 == 256 { r as i32 } else { (r as i32 * gain_q8) >> 8 };
        *o = (p as i32 + r).clamp(0, 255) as u8;
    }
}

/// Reconstruct a picture: `pred + g × residual`, clamped.
pub fn reconstruct(pred: &Planes, res: &Residual, gain_q8: i32) -> Planes {
    let mut o = Planes::grey(pred.w, pred.h);
    add_clamp(&pred.y, &res.y, gain_q8, &mut o.y);
    add_clamp(&pred.cb, &res.cb, gain_q8, &mut o.cb);
    add_clamp(&pred.cr, &res.cr, gain_q8, &mut o.cr);
    o
}

/// Encode `src` against the encoder's previous reconstruction (`None` — an intra frame). Closed
/// loop: the returned reconstruction is exactly what an intact decoder makes of the packet.
pub fn encode(src: &Planes, prev: Option<&Planes>, cfg: &Cfg) -> Clean {
    let (w, h) = (src.w, src.h);
    let q = cfg.qstep();
    let (intra, mv, pred) = match prev {
        None => (true, vec![(0, 0); (w / cfg.bs) * (h / cfg.bs)], Planes::grey(w, h)),
        Some(rf) => {
            let mv = estimate(src, rf, cfg);
            let pred = predict(rf, &mv, cfg.bs);
            (false, mv, pred)
        }
    };
    let res = Residual {
        y: code_residual(&src.y, &pred.y, w, h, q),
        cb: code_residual(&src.cb, &pred.cb, w / 2, h / 2, q),
        cr: code_residual(&src.cr, &pred.cr, w / 2, h / 2, q),
    };
    let recon = reconstruct(&pred, &res, 256);
    Clean { recon, pkt: Packet { intra, mv, res } }
}

/// Zero the residual of luma block `i` (and its chroma) — a lost macroblock.
pub fn drop_block_residual(res: &mut Residual, w: usize, bs: usize, i: usize) {
    let bw = w / bs;
    let (bx, by) = ((i % bw) * bs, (i / bw) * bs);
    for y in 0..bs {
        res.y[(by + y) * w + bx..(by + y) * w + bx + bs].fill(0);
    }
    let (cs, cw) = (bs / 2, w / 2);
    for y in 0..cs {
        let o = (by / 2 + y) * cw + bx / 2;
        res.cb[o..o + cs].fill(0);
        res.cr[o..o + cs].fill(0);
    }
}
