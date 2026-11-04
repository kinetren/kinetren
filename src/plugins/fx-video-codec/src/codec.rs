//! The block codec (datamosh-codec-path, design D7): Y'CbCr 4:2:0 planes, block-matching motion
//! estimation, a closed prediction loop, an 8×8 DCT residual quantised by QP. No entropy coding,
//! no B-frames, integer-pel vectors — the MECHANISM of an inter-frame codec, not a standard's syntax.
//!
//! The motion search, the residual and the decoder are compute shaders (`wgsl/`, compute-kernels):
//! `encode` and `decode` are jobs for the host's device, and `None` means the device failed. The same machine gives the same
//! bytes; another device may differ, the codec's look does not depend on it.

use kt_plugin_api::compute::{self, Buf, Init, Job, Step};
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
// encode / decode
// ---------------------------------------------------------------------------------------------

/// The residual shader with the DCT-II basis `C[u][x]` as its table.
fn residual_shader() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| {
        let mut c = String::from("var<private> C: array<f32, 64> = array<f32, 64>(");
        for u in 0..8 {
            let k = if u == 0 { 0.353_553_390_593_273_73 } else { 0.5 };
            for x in 0..8 {
                c += &format!("{:.9},", k * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).fcos());
            }
        }
        format!("{c});\n{}", include_str!("wgsl/residual.wgsl"))
    })
}

fn words(v: &[u8]) -> Vec<u8> {
    v.iter().flat_map(|b| (*b as u32).to_le_bytes()).collect()
}

fn uniform(v: &[[u8; 4]]) -> Vec<u8> {
    v.concat()
}

fn i16s(v: &[u8]) -> Vec<i16> {
    v.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i16).collect()
}

fn narrow(v: &[u8]) -> Vec<u8> {
    v.chunks_exact(4).map(|c| c[0]).collect()
}

fn wide(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|r| (*r as i32).to_le_bytes()).collect()
}

fn decode_uniforms(w: u32, h: u32, bs: u32, gain_q8: i32, intra: bool) -> [Vec<u8>; 3] {
    let u = |pw: u32, ph: u32, shift: u32| {
        uniform(&[pw.to_le_bytes(), ph.to_le_bytes(), bs.to_le_bytes(), shift.to_le_bytes(), gain_q8.to_le_bytes(), (w / bs).to_le_bytes(), (intra as u32).to_le_bytes(), [0; 4]])
    };
    [u(w, h, 0), u(w / 2, h / 2, 1), u(w / 2, h / 2, 1)]
}

fn decode_step<'a>(u: &'a [u8], read: &'a [Buf], write: &'a [Buf], groups: [u32; 3]) -> Step<'a> {
    Step { shader: include_str!("wgsl/decode.wgsl"), uniform: u, read, write, groups }
}

/// Decode a picture: the prediction from `rf` by the vector field (one vector per luma block of
/// `bs`; the chroma block of `bs/2` takes the vector halved, floored) plus `g × residual`, clamped.
/// `None`: as `encode`.
pub fn decode(rf: &Planes, mv: &[(i16, i16)], bs: usize, res: &Residual, gain_q8: i32) -> Option<Planes> {
    let host = compute::host()?;
    let (w, h, b) = (rf.w as u32, rf.h as u32, bs as u32);
    let (bw, bh) = (w / b, h / b);
    let vectors: Vec<u8> = mv.iter().flat_map(|v| [(v.0 as i32).to_le_bytes(), (v.1 as i32).to_le_bytes()].concat()).collect();
    let rf_words = [words(&rf.y), words(&rf.cb), words(&rf.cr)];
    let res_words = [wide(&res.y), wide(&res.cb), wide(&res.cr)];
    let plane = |i: usize| res_words[i].len() as u64;
    // 0–2 the reference, 3 the vectors, 4–6 the residual, 7–9 the picture
    let buffers = [
        Init::Bytes(&rf_words[0]), Init::Bytes(&rf_words[1]), Init::Bytes(&rf_words[2]),
        Init::Bytes(&vectors),
        Init::Bytes(&res_words[0]), Init::Bytes(&res_words[1]), Init::Bytes(&res_words[2]),
        Init::Zero(plane(0)), Init::Zero(plane(1)), Init::Zero(plane(2)),
    ];
    let us = decode_uniforms(w, h, b, gain_q8, false);
    let reads = [[Buf(0), Buf(3), Buf(4)], [Buf(1), Buf(3), Buf(5)], [Buf(2), Buf(3), Buf(6)]];
    let writes = [[Buf(7)], [Buf(8)], [Buf(9)]];
    let groups = [bw.div_ceil(8), bh.div_ceil(8), 1];
    let steps = [
        decode_step(&us[0], &reads[0], &writes[0], groups),
        decode_step(&us[1], &reads[1], &writes[1], groups),
        decode_step(&us[2], &reads[2], &writes[2], groups),
    ];
    let got = match host.run(&Job { buffers: &buffers, steps: &steps, reads: &[Buf(7), Buf(8), Buf(9)] }) {
        Ok(g) => g,
        Err(e) => {
            compute::fail(e);
            return None;
        }
    };
    Some(Planes { w: rf.w, h: rf.h, y: narrow(&got[0]), cb: narrow(&got[1]), cr: narrow(&got[2]) })
}

/// Encode `src` against the encoder's previous reconstruction (`None` — an intra frame). Closed
/// loop: the returned reconstruction is exactly what an intact decoder makes of the packet. `None`
/// — no device, or the device failed (`compute::unavailable` says why).
pub fn encode(src: &Planes, prev: Option<&Planes>, cfg: &Cfg) -> Option<Clean> {
    let host = compute::host()?;
    let (w, h, bs) = (src.w as u32, src.h as u32, cfg.bs as u32);
    let (bw, bh) = (w / bs, h / bs);
    let mv_bytes = (bw * bh * 8) as u64;
    let q = cfg.qstep() as f32;
    let cur = [words(&src.y), words(&src.cb), words(&src.cr)];
    let rf = prev.map(|p| [words(&p.y), words(&p.cb), words(&p.cr)]);
    let rf_init = |i: usize| rf.as_ref().map_or(Init::Zero(16), |r| Init::Bytes(&r[i]));
    // 0–2 the source, 3–5 the reference, 6 the zero seed, 7–8 the two sweeps' vectors, 9–11 the
    // residual, 12–14 the reconstruction
    let plane_bytes = [(w * h * 4) as u64, (w * h) as u64, (w * h) as u64];
    let buffers = [
        Init::Bytes(&cur[0]), Init::Bytes(&cur[1]), Init::Bytes(&cur[2]),
        rf_init(0), rf_init(1), rf_init(2),
        Init::Zero(mv_bytes), Init::Zero(mv_bytes), Init::Zero(mv_bytes),
        Init::Zero(plane_bytes[0]), Init::Zero(plane_bytes[1]), Init::Zero(plane_bytes[2]),
        Init::Zero(plane_bytes[0]), Init::Zero(plane_bytes[1]), Init::Zero(plane_bytes[2]),
    ];
    let search_u = uniform(&[w.to_le_bytes(), h.to_le_bytes(), bs.to_le_bytes(), cfg.search.max(0).to_le_bytes()]);
    let residual_u = |pw: u32, ph: u32, shift: u32| {
        uniform(&[pw.to_le_bytes(), ph.to_le_bytes(), bs.to_le_bytes(), shift.to_le_bytes(), q.to_le_bytes(), bw.to_le_bytes(), (prev.is_none() as u32).to_le_bytes(), [0; 4]])
    };
    let us = [residual_u(w, h, 0), residual_u(w / 2, h / 2, 1), residual_u(w / 2, h / 2, 1)];
    let search = include_str!("wgsl/search.wgsl");
    let groups = [bw.div_ceil(8), bh.div_ceil(8), 1];
    let mut steps = Vec::new();
    if prev.is_some() {
        steps.push(Step { shader: search, uniform: &search_u, read: &[Buf(0), Buf(3), Buf(6)], write: &[Buf(7)], groups });
        steps.push(Step { shader: search, uniform: &search_u, read: &[Buf(0), Buf(3), Buf(7)], write: &[Buf(8)], groups });
    }
    let reads = [[Buf(0), Buf(3), Buf(8)], [Buf(1), Buf(4), Buf(8)], [Buf(2), Buf(5), Buf(8)]];
    let writes = [[Buf(9)], [Buf(10)], [Buf(11)]];
    for (i, pw) in [w, w / 2, w / 2].into_iter().enumerate() {
        let ph = if i == 0 { h } else { h / 2 };
        steps.push(Step {
            shader: residual_shader(),
            uniform: &us[i],
            read: &reads[i],
            write: &writes[i],
            groups: [(pw / 8).div_ceil(8), (ph / 8).div_ceil(8), 1],
        });
    }
    let ds = decode_uniforms(w, h, bs, 256, prev.is_none());
    let dreads = [[Buf(3), Buf(8), Buf(9)], [Buf(4), Buf(8), Buf(10)], [Buf(5), Buf(8), Buf(11)]];
    let dwrites = [[Buf(12)], [Buf(13)], [Buf(14)]];
    for i in 0..3 {
        steps.push(decode_step(&ds[i], &dreads[i], &dwrites[i], groups));
    }
    let got = match host.run(&Job { buffers: &buffers, steps: &steps, reads: &[Buf(8), Buf(9), Buf(10), Buf(11), Buf(12), Buf(13), Buf(14)] }) {
        Ok(g) => g,
        Err(e) => {
            compute::fail(e);
            return None;
        }
    };
    let mv: Vec<(i16, i16)> = got[0]
        .chunks_exact(8)
        .map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i16, i32::from_le_bytes([c[4], c[5], c[6], c[7]]) as i16))
        .collect();
    let res = Residual { y: i16s(&got[1]), cb: i16s(&got[2]), cr: i16s(&got[3]) };
    let recon = Planes { w: src.w, h: src.h, y: narrow(&got[4]), cb: narrow(&got[5]), cr: narrow(&got[6]) };
    Some(Clean { recon, pkt: Packet { intra: prev.is_none(), mv, res } })
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
