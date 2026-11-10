//! The block codec (datamosh-codec-path, design D7): Y'CbCr 4:2:0 planes, block-matching motion
//! estimation, a closed prediction loop, an 8×8 DCT residual quantised by QP. No entropy coding,
//! no B-frames, integer-pel vectors — the MECHANISM of an inter-frame codec, not a standard's syntax.
//!
//! The colour conversion, the motion search, the residual and the decoder are compute shaders
//! (`wgsl/`, compute-kernels; the conversion of the source since stream-layout-per-build,
//! `Planes::from_rgba` staying as the CPU reference):
//! `encode` and `decode` are jobs for the host's device, and `None` means the device failed. The same machine gives the same
//! bytes; another device may differ, the codec's look does not depend on it.

use kt_plugin_api::compute::{self, Buf, Init, Job, Step};
use kt_plugin_api::math::FrozenMath;
use std::collections::HashMap;
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
    include_str!("wgsl/residual.wgsl")
}

/// The DCT basis as the residual's uniform carries it (gpu-pass-cost): row u = frequency. Each value
/// is the decimal the shader once held as a literal, rounded as naga rounds an abstract float to
/// `f32` (through `f64`), so the products are the same bits.
fn dct_basis() -> &'static [u8] {
    static B: OnceLock<Vec<u8>> = OnceLock::new();
    B.get_or_init(|| {
        let mut out = Vec::with_capacity(256);
        for u in 0..8 {
            let k = if u == 0 { 0.353_553_390_593_273_73 } else { 0.5 };
            for x in 0..8 {
                let text = format!("{:.9}", k * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).fcos());
                out.extend_from_slice(&(text.parse::<f64>().unwrap() as f32).to_le_bytes());
            }
        }
        out
    })
}

/// Every compute shader of the codec, as the device receives it, with the buffers a step of it reads
/// and writes.
pub fn shaders() -> [(&'static str, usize, usize); 6] {
    [
        (CONVERT, 1, 3),
        (SEARCH, 3, 1),
        (residual_shader(), 3, 1),
        (DECODE, 3, 1),
        (VECTORS, 2, 2),
        (DROP, 2, 1),
    ]
}

const SEARCH: &str = include_str!("wgsl/search.wgsl");
const DECODE: &str = include_str!("wgsl/decode.wgsl");
/// The broken decoder's vector edit (stream-history-on-device D3).
const VECTORS: &str = include_str!("wgsl/vectors.wgsl");
/// The residual of lost blocks zeroed (stream-history-on-device D3).
const DROP: &str = include_str!("wgsl/drop.wgsl");

const CONVERT: &str = include_str!("wgsl/convert.wgsl");

/// A plane as the shaders read it: its bytes, four samples to a word, padded to a whole word
/// (gpu-pass-cost: a word per sample had quadrupled every upload and cost a pass over the plane).
fn packed(v: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if v.len().is_multiple_of(4) {
        return std::borrow::Cow::Borrowed(v);
    }
    let mut p = v.to_vec();
    p.resize(v.len().next_multiple_of(4), 0);
    std::borrow::Cow::Owned(p)
}

fn uniform(v: &[[u8; 4]]) -> Vec<u8> {
    v.concat()
}

/// A residual plane as the shaders read and write it: two 16-bit samples to a word
/// (stream-layout-per-build: a 32-bit word a sample doubled the residual's upload and readback).
fn pack16(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|r| r.to_le_bytes()).collect()
}

fn unpack16(v: &[u8]) -> Vec<i16> {
    v.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
}

fn decode_uniforms(w: u32, h: u32, bs: u32, gain_q8: i32, intra: bool) -> [Vec<u8>; 3] {
    let u = |pw: u32, ph: u32, shift: u32| {
        uniform(&[pw.to_le_bytes(), ph.to_le_bytes(), bs.to_le_bytes(), shift.to_le_bytes(), gain_q8.to_le_bytes(), (w / bs).to_le_bytes(), (intra as u32).to_le_bytes(), [0; 4]])
    };
    [u(w, h, 0), u(w / 2, h / 2, 1), u(w / 2, h / 2, 1)]
}

/// The residual's uniforms of the three planes (the DCT basis as their table).
fn residual_uniforms(w: u32, h: u32, bs: u32, q: f32, intra: bool) -> [Vec<u8>; 3] {
    let u = |pw: u32, ph: u32, shift: u32| {
        let mut u = uniform(&[pw.to_le_bytes(), ph.to_le_bytes(), bs.to_le_bytes(), shift.to_le_bytes(), q.to_le_bytes(), (w / bs).to_le_bytes(), (intra as u32).to_le_bytes(), [0; 4]]);
        u.extend_from_slice(dct_basis());
        u
    };
    [u(w, h, 0), u(w / 2, h / 2, 1), u(w / 2, h / 2, 1)]
}

fn decode_step<'a>(u: &'a [u8], read: &'a [Buf], write: &'a [Buf], groups: [u32; 3]) -> Step<'a> {
    Step { name: "datamosh:decode", shader: include_str!("wgsl/decode.wgsl"), uniform: u, read, write, groups }
}

/// Decode a picture: the prediction from `rf` by the vector field (one vector per luma block of
/// `bs`; the chroma block of `bs/2` takes the vector halved, floored) plus `g × residual`, clamped.
/// `None`: as `encode`.
pub fn decode(rf: &Planes, mv: &[(i16, i16)], bs: usize, res: &Residual, gain_q8: i32) -> Option<Planes> {
    let host = compute::host()?;
    let (w, h, b) = (rf.w as u32, rf.h as u32, bs as u32);
    let (bw, bh) = (w / b, h / b);
    let vectors: Vec<u8> = mv.iter().flat_map(|v| [(v.0 as i32).to_le_bytes(), (v.1 as i32).to_le_bytes()].concat()).collect();
    let rf_words = [packed(&rf.y), packed(&rf.cb), packed(&rf.cr)];
    let res_words = [pack16(&res.y), pack16(&res.cb), pack16(&res.cr)];
    let plane = |i: usize| rf_words[i].len() as u64;
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
    let mut got = got.into_iter();
    let mut next = || got.next().unwrap_or_default();
    Some(Planes { w: rf.w, h: rf.h, y: next(), cb: next(), cr: next() })
}

/// What an encoder job starts from: planes already converted, or the source frame's RGBA, which the
/// job converts first (stream-layout-per-build).
#[derive(Clone, Copy)]
pub enum Source<'a> {
    Planes(&'a Planes),
    /// `sw`×`sh` RGBA into padded planes `w`×`h`.
    Rgba { rgba: &'a [u8], sw: u32, sh: u32, w: usize, h: usize },
}

/// Encode `src` against the encoder's previous reconstruction (`None` — an intra frame). Closed
/// loop: the returned reconstruction is exactly what an intact decoder makes of the packet. `None`
/// — no device, or the device failed (`compute::unavailable` says why).
pub fn encode(src: &Planes, prev: Option<&Planes>, cfg: &Cfg) -> Option<Clean> {
    encode_from(Source::Planes(src), prev, cfg)
}

/// `encode` of a source frame `sw`×`sh` given as RGBA: the conversion to the padded planes `w`×`h`
/// is the job's first step, the same bytes as `Planes::from_rgba` (t2274).
pub fn encode_rgba(rgba: &[u8], sw: u32, sh: u32, w: usize, h: usize, prev: Option<&Planes>, cfg: &Cfg) -> Option<Clean> {
    if sw == 0 || sh == 0 || rgba.len() < sw as usize * sh as usize * 4 {
        return encode(&Planes::from_rgba(rgba, sw, sh, w, h), prev, cfg);
    }
    encode_from(Source::Rgba { rgba: &rgba[..sw as usize * sh as usize * 4], sw, sh, w, h }, prev, cfg)
}

/// `encode` from either source.
pub fn encode_from(src: Source<'_>, prev: Option<&Planes>, cfg: &Cfg) -> Option<Clean> {
    let host = compute::host()?;
    let (pw, ph) = match src {
        Source::Planes(p) => (p.w, p.h),
        Source::Rgba { w, h, .. } => (w, h),
    };
    let (w, h, bs) = (pw as u32, ph as u32, cfg.bs as u32);
    let (bw, bh) = (w / bs, h / bs);
    let mv_bytes = (bw * bh * 8) as u64;
    let q = cfg.qstep() as f32;
    let cur = match src {
        Source::Planes(p) => Some([packed(&p.y), packed(&p.cb), packed(&p.cr)]),
        Source::Rgba { .. } => None,
    };
    let rf = prev.map(|p| [packed(&p.y), packed(&p.cb), packed(&p.cr)]);
    let rf_init = |i: usize| rf.as_ref().map_or(Init::Zero(16), |r| Init::Bytes(&r[i]));
    // the planes' bytes, the residual's (two to a sample)
    let planes = [(w * h) as u64, (w * h / 4) as u64, (w * h / 4) as u64];
    let cur_init = |i: usize| cur.as_ref().map_or(Init::Zero(planes[i]), |c| Init::Bytes(&c[i]));
    // 0–2 the source, 3–5 the reference, 6 the zero seed, 7–8 the two sweeps' vectors, 9–11 the
    // residual, 12–14 the reconstruction, 15 the source's RGBA when the job converts it
    let mut buffers = vec![
        cur_init(0), cur_init(1), cur_init(2),
        rf_init(0), rf_init(1), rf_init(2),
        Init::Zero(mv_bytes), Init::Zero(mv_bytes), Init::Zero(mv_bytes),
        Init::Zero(planes[0] * 2), Init::Zero(planes[1] * 2), Init::Zero(planes[2] * 2),
        Init::Zero(planes[0]), Init::Zero(planes[1]), Init::Zero(planes[2]),
    ];
    let convert_u = match src {
        Source::Rgba { rgba, sw, sh, .. } => {
            buffers.push(Init::Bytes(rgba));
            Some(uniform(&[sw.to_le_bytes(), sh.to_le_bytes(), w.to_le_bytes(), h.to_le_bytes()]))
        }
        Source::Planes(_) => None,
    };
    let search_u = uniform(&[w.to_le_bytes(), h.to_le_bytes(), bs.to_le_bytes(), cfg.search.max(0).to_le_bytes()]);
    let us = residual_uniforms(w, h, bs, q, prev.is_none());
    let search = include_str!("wgsl/search.wgsl");
    let groups = [bw.div_ceil(8), bh.div_ceil(8), 1];
    let mut steps = Vec::new();
    if let Some(u) = &convert_u {
        // one invocation per 8×2 luma samples
        steps.push(Step { name: "datamosh:convert", shader: CONVERT, uniform: u, read: &[Buf(15)], write: &[Buf(0), Buf(1), Buf(2)], groups: [(w / 8).div_ceil(8), (h / 2).div_ceil(8), 1] });
    }
    if prev.is_some() {
        steps.push(Step { name: "datamosh:search", shader: search, uniform: &search_u, read: &[Buf(0), Buf(3), Buf(6)], write: &[Buf(7)], groups });
        steps.push(Step { name: "datamosh:search", shader: search, uniform: &search_u, read: &[Buf(0), Buf(3), Buf(7)], write: &[Buf(8)], groups });
    }
    let reads = [[Buf(0), Buf(3), Buf(8)], [Buf(1), Buf(4), Buf(8)], [Buf(2), Buf(5), Buf(8)]];
    let writes = [[Buf(9)], [Buf(10)], [Buf(11)]];
    for (i, pw) in [w, w / 2, w / 2].into_iter().enumerate() {
        let ph = if i == 0 { h } else { h / 2 };
        steps.push(Step {
            name: "datamosh:residual",
            shader: residual_shader(),
            uniform: &us[i],
            read: &reads[i],
            write: &writes[i],
            // a workgroup per 8×8 block
            groups: [pw / 8, ph / 8, 1],
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
    let mut got = got.into_iter();
    let mut next = || got.next().unwrap_or_default();
    let mv: Vec<(i16, i16)> = next()
        .chunks_exact(8)
        .map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i16, i32::from_le_bytes([c[4], c[5], c[6], c[7]]) as i16))
        .collect();
    let res = Residual { y: unpack16(&next()), cb: unpack16(&next()), cr: unpack16(&next()) };
    let recon = Planes { w: pw, h: ph, y: next(), cb: next(), cr: next() };
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

// ---------------------------------------------------------------------------------------------
// the batched walk's device program (stream-history-on-device D1)
// ---------------------------------------------------------------------------------------------

/// Three planes inside a [`Batch`]: the job's buffer indices (luma, Cb, Cr).
pub type Dev3 = [usize; 3];

/// A clean encoder's output inside a batch: its reconstruction, its vectors (`None` — intra, zero
/// vectors) and its residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevClean {
    pub recon: Dev3,
    pub mv: Option<usize>,
    pub res: Dev3,
}

/// The macroblock loss of one position, in the integers the device computes it with (D3): a block
/// is lost when its hash is below `thr`, or always (`all`, loss 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loss {
    pub thr: u32,
    pub all: bool,
    pub seed: u32,
    pub k: u32,
    /// The search range: a lost block's random vector lies within it.
    pub rr: i32,
}

impl Loss {
    /// `h < loss · 2³²` for an integer `h` is `h < ⌈loss · 2³²⌉`; from 2³² on every block is lost.
    pub fn of(loss: f64, seed: u32, k: i64, rr: i32) -> Option<Loss> {
        if loss <= 0.0 {
            return None;
        }
        let l = (loss * 4_294_967_296.0).ceil();
        let all = l >= 4_294_967_296.0;
        Some(Loss { thr: if all { u32::MAX } else { l as u32 }, all, seed, k: k as u32, rr })
    }
}

/// The search range of the vector table: every vector the search gives lies within it (the search
/// clamps to its range, at most 64 px).
pub const TABLE_RANGE: i32 = 64;

enum Init0 {
    Bytes(Vec<u8>),
    Zero(u64),
}

struct StepRec {
    name: &'static str,
    shader: &'static str,
    uniform: Vec<u8>,
    read: Vec<Buf>,
    write: Vec<Buf>,
    groups: [u32; 3],
}

/// One submission of a history walk: the codec's steps over many positions, their buffers on the
/// device between positions (D1). A buffer a step writes is handed out again once the walk says it
/// is no longer live (`sweep`); an upload, a shared zero buffer and a buffer read back never are
/// (`write_buffer` lands before the whole submission; reads are copied at its end).
pub struct Batch {
    w: usize,
    h: usize,
    bufs: Vec<Init0>,
    fixed: Vec<bool>,
    free: HashMap<u64, Vec<usize>>,
    in_use: Vec<usize>,
    zeros: HashMap<u64, usize>,
    steps: Vec<StepRec>,
    reads: Vec<usize>,
    read_at: HashMap<usize, usize>,
    /// Bytes uploaded and allocated: the device memory the batch holds.
    pub bytes: usize,
}

fn words(v: &[u8]) -> Vec<u8> {
    packed(v).into_owned()
}

impl Batch {
    /// A batch for padded planes `w`×`h`.
    pub fn new(w: usize, h: usize) -> Batch {
        Batch {
            w,
            h,
            bufs: Vec::new(),
            fixed: Vec::new(),
            free: HashMap::new(),
            in_use: Vec::new(),
            zeros: HashMap::new(),
            steps: Vec::new(),
            reads: Vec::new(),
            read_at: HashMap::new(),
            bytes: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty() && self.reads.is_empty()
    }

    pub fn steps(&self) -> usize {
        self.steps.len()
    }

    fn size(&self, id: usize) -> u64 {
        match &self.bufs[id] {
            Init0::Bytes(b) => b.len() as u64,
            Init0::Zero(n) => *n,
        }
    }

    fn alloc(&mut self, n: u64) -> usize {
        if let Some(id) = self.free.get_mut(&n).and_then(Vec::pop) {
            self.in_use.push(id);
            return id;
        }
        self.bufs.push(Init0::Zero(n));
        self.fixed.push(false);
        self.bytes += n as usize;
        let id = self.bufs.len() - 1;
        self.in_use.push(id);
        id
    }

    fn zero(&mut self, n: u64) -> usize {
        if let Some(&id) = self.zeros.get(&n) {
            return id;
        }
        self.bufs.push(Init0::Zero(n));
        self.fixed.push(true);
        self.bytes += n as usize;
        let id = self.bufs.len() - 1;
        self.zeros.insert(n, id);
        id
    }

    /// Bytes the device receives with the submission.
    pub fn upload(&mut self, v: Vec<u8>) -> usize {
        self.bytes += v.len();
        self.bufs.push(Init0::Bytes(v));
        self.fixed.push(true);
        self.bufs.len() - 1
    }

    pub fn upload_planes(&mut self, p: &Planes) -> Dev3 {
        [self.upload(words(&p.y)), self.upload(words(&p.cb)), self.upload(words(&p.cr))]
    }

    /// A clean packet held on the host: its vectors (32-bit pairs) and its residual.
    pub fn upload_packet(&mut self, pkt: &Packet) -> (Option<usize>, Dev3) {
        let mv = (!pkt.intra).then(|| {
            self.upload(pkt.mv.iter().flat_map(|v| [(v.0 as i32).to_le_bytes(), (v.1 as i32).to_le_bytes()].concat()).collect())
        });
        (mv, [self.upload(pack16(&pkt.res.y)), self.upload(pack16(&pkt.res.cb)), self.upload(pack16(&pkt.res.cr))])
    }

    fn planes(&mut self) -> Dev3 {
        let (y, c) = ((self.w * self.h) as u64, (self.w * self.h / 4) as u64);
        [self.alloc(y), self.alloc(c), self.alloc(c)]
    }

    fn residual(&mut self) -> Dev3 {
        let (y, c) = ((self.w * self.h * 2) as u64, (self.w * self.h / 2) as u64);
        [self.alloc(y), self.alloc(c), self.alloc(c)]
    }

    fn mv_bytes(&self, bs: usize) -> u64 {
        ((self.w / bs) * (self.h / bs) * 8) as u64
    }

    /// Zero vectors for block size `bs` (an intra packet of the second input carries no motion).
    pub fn zero_mv(&mut self, bs: usize) -> usize {
        let n = self.mv_bytes(bs);
        self.zero(n)
    }

    fn step(&mut self, name: &'static str, shader: &'static str, uniform: Vec<u8>, read: &[usize], write: &[usize], groups: [u32; 3]) {
        self.steps.push(StepRec {
            name,
            shader,
            uniform,
            read: read.iter().map(|&i| Buf(i)).collect(),
            write: write.iter().map(|&i| Buf(i)).collect(),
            groups,
        });
    }

    /// Bring buffers back with the submission; their index in `run`'s answer. A buffer read is never
    /// handed out again.
    pub fn read(&mut self, id: usize) -> usize {
        if let Some(&i) = self.read_at.get(&id) {
            return i;
        }
        self.fixed[id] = true;
        self.reads.push(id);
        self.read_at.insert(id, self.reads.len() - 1);
        self.reads.len() - 1
    }

    pub fn read3(&mut self, d: Dev3) -> [usize; 3] {
        [self.read(d[0]), self.read(d[1]), self.read(d[2])]
    }

    /// Where a buffer read comes in `run`'s answer.
    pub fn read_index(&self, id: usize) -> usize {
        self.read_at.get(&id).copied().unwrap_or(usize::MAX)
    }

    pub fn read_index3(&self, d: Dev3) -> [usize; 3] {
        [self.read_index(d[0]), self.read_index(d[1]), self.read_index(d[2])]
    }

    /// Whether `id` comes back.
    pub fn is_read(&self, id: usize) -> bool {
        self.read_at.contains_key(&id)
    }

    /// Hand out again every buffer a step wrote that is neither `live` nor read.
    pub fn sweep(&mut self, live: &dyn Fn(usize) -> bool) {
        let mut keep = Vec::with_capacity(self.in_use.len());
        for id in std::mem::take(&mut self.in_use) {
            if self.fixed[id] || live(id) {
                if !self.fixed[id] {
                    keep.push(id);
                }
            } else {
                let n = self.size(id);
                self.free.entry(n).or_default().push(id);
            }
        }
        self.in_use = keep;
    }

    /// The source frame `sw`×`sh` as RGBA into the padded planes (`Planes::from_rgba` on the device).
    pub fn convert(&mut self, rgba: Vec<u8>, sw: u32, sh: u32) -> Dev3 {
        let (w, h) = (self.w as u32, self.h as u32);
        let src = self.upload(rgba);
        let out = self.planes();
        let u = uniform(&[sw.to_le_bytes(), sh.to_le_bytes(), w.to_le_bytes(), h.to_le_bytes()]);
        self.step("datamosh:convert", CONVERT, u, &[src], &out, [(w / 8).div_ceil(8), (h / 2).div_ceil(8), 1]);
        out
    }

    /// The clean encoder at one position: `cur` against `prev` (`None` — an intra frame). The steps of
    /// `encode_from`, in its order, on the batch's buffers.
    pub fn encode(&mut self, cur: Dev3, prev: Option<Dev3>, cfg: &Cfg) -> DevClean {
        let (w, h, bs) = (self.w as u32, self.h as u32, cfg.bs as u32);
        let (bw, bh) = (w / bs, h / bs);
        let groups = [bw.div_ceil(8), bh.div_ceil(8), 1];
        let zero_mv = self.zero_mv(cfg.bs);
        let rf = match prev {
            Some(p) => p,
            None => {
                let z = self.zero(16);
                [z, z, z]
            }
        };
        let mv = match prev {
            Some(p) => {
                let su = uniform(&[w.to_le_bytes(), h.to_le_bytes(), bs.to_le_bytes(), cfg.search.max(0).to_le_bytes()]);
                let n = self.mv_bytes(cfg.bs);
                let (m1, m2) = (self.alloc(n), self.alloc(n));
                self.step("datamosh:search", SEARCH, su.clone(), &[cur[0], p[0], zero_mv], &[m1], groups);
                self.step("datamosh:search", SEARCH, su, &[cur[0], p[0], m1], &[m2], groups);
                Some(m2)
            }
            None => None,
        };
        let mvb = mv.unwrap_or(zero_mv);
        let res = self.residual();
        let us = residual_uniforms(w, h, bs, cfg.qstep() as f32, prev.is_none());
        for (i, u) in us.into_iter().enumerate() {
            let (pw, ph) = if i == 0 { (w, h) } else { (w / 2, h / 2) };
            self.step("datamosh:residual", residual_shader(), u, &[cur[i], rf[i], mvb], &[res[i]], [pw / 8, ph / 8, 1]);
        }
        let recon = self.planes();
        let ds = decode_uniforms(w, h, bs, 256, prev.is_none());
        for (i, u) in ds.into_iter().enumerate() {
            self.step("datamosh:decode", DECODE, u, &[rf[i], mvb, res[i]], &[recon[i]], groups);
        }
        DevClean { recon, mv, res }
    }

    /// The broken decoder's vector edit: `src` through the table (`None` — the identity) and the loss.
    /// Returns the edited vectors and the lost blocks' marks (`None` — no loss).
    pub fn edit(&mut self, src: usize, bs: usize, table: Option<usize>, loss: Option<Loss>) -> (usize, Option<usize>) {
        let n = ((self.w / bs) * (self.h / bs)) as u32;
        let mv = self.alloc(self.mv_bytes(bs));
        let lost = self.alloc(n as u64 * 4);
        let table_buf = match table {
            Some(t) => t,
            None => self.zero(16),
        };
        let flags = table.is_some() as u32 | loss.map_or(0, |l| 2 | if l.all { 4 } else { 0 });
        let l = loss.unwrap_or(Loss { thr: 0, all: false, seed: 0, k: 0, rr: 0 });
        let u = uniform(&[
            n.to_le_bytes(),
            flags.to_le_bytes(),
            l.thr.to_le_bytes(),
            l.seed.to_le_bytes(),
            l.k.to_le_bytes(),
            l.rr.to_le_bytes(),
            TABLE_RANGE.to_le_bytes(),
            [0; 4],
        ]);
        self.step("datamosh:vectors", VECTORS, u, &[src, table_buf], &[mv, lost], [n.div_ceil(64), 1, 1]);
        (mv, loss.map(|_| lost))
    }

    /// The residual with the lost blocks' samples zeroed, a copy (the packet's own stays intact).
    pub fn drop_lost(&mut self, res: Dev3, lost: usize, bs: usize) -> Dev3 {
        let out = self.residual();
        let mw = (self.w / bs) as u32;
        for i in 0..3 {
            let (pw, ph, shift) = if i == 0 { (self.w as u32, self.h as u32, 0) } else { (self.w as u32 / 2, self.h as u32 / 2, 1) };
            let u = uniform(&[pw.to_le_bytes(), ph.to_le_bytes(), ((bs as u32) >> shift).to_le_bytes(), mw.to_le_bytes()]);
            self.step("datamosh:drop", DROP, u, &[res[i], lost], &[out[i]], [(pw * ph / 2).div_ceil(64), 1, 1]);
        }
        out
    }

    /// A P-frame of the decoder: the prediction from `rf` by `mv` plus `gain` × `res`.
    pub fn decode(&mut self, rf: Dev3, mv: usize, res: Dev3, bs: usize, gain_q8: i32) -> Dev3 {
        let (w, h, b) = (self.w as u32, self.h as u32, bs as u32);
        let groups = [(w / b).div_ceil(8), (h / b).div_ceil(8), 1];
        let out = self.planes();
        for (i, u) in decode_uniforms(w, h, b, gain_q8, false).into_iter().enumerate() {
            self.step("datamosh:decode", DECODE, u, &[rf[i], mv, res[i]], &[out[i]], groups);
        }
        out
    }

    /// Submit the batch and wait once; the buffers read, in `read`'s order. `None` — no device, or
    /// it failed (`compute::fail` says why).
    pub fn run(&self) -> Option<Vec<Vec<u8>>> {
        let host = compute::host()?;
        let inits: Vec<Init<'_>> = self
            .bufs
            .iter()
            .map(|b| match b {
                Init0::Bytes(v) => Init::Bytes(v),
                Init0::Zero(n) => Init::Zero(*n),
            })
            .collect();
        let steps: Vec<Step<'_>> = self
            .steps
            .iter()
            .map(|s| Step { name: s.name, shader: s.shader, uniform: &s.uniform, read: &s.read, write: &s.write, groups: s.groups })
            .collect();
        let reads: Vec<Buf> = self.reads.iter().map(|&i| Buf(i)).collect();
        match host.run(&Job { buffers: &inits, steps: &steps, reads: &reads }) {
            Ok(g) => Some(g),
            Err(e) => {
                compute::fail(e);
                None
            }
        }
    }

    /// Planes from three buffers `run` brought back.
    pub fn planes_of(&self, got: &[Vec<u8>], at: [usize; 3]) -> Planes {
        let take = |i: usize, n: usize| {
            let mut v = got.get(at[i]).cloned().unwrap_or_default();
            v.truncate(n);
            v
        };
        let (y, c) = (self.w * self.h, self.w * self.h / 4);
        Planes { w: self.w, h: self.h, y: take(0, y), cb: take(1, c), cr: take(2, c) }
    }

    /// A clean output from the buffers `run` brought back (vectors, residual, reconstruction).
    pub fn clean_of(&self, got: &[Vec<u8>], mv: Option<usize>, res: [usize; 3], recon: [usize; 3], bs: usize) -> Clean {
        let intra = mv.is_none();
        let mv: Vec<(i16, i16)> = match mv {
            Some(i) => got
                .get(i)
                .map(|v| v.chunks_exact(8).map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i16, i32::from_le_bytes([c[4], c[5], c[6], c[7]]) as i16)).collect())
                .unwrap_or_default(),
            None => vec![(0, 0); (self.w / bs) * (self.h / bs)],
        };
        let r = |i: usize| unpack16(got.get(res[i]).map_or(&[][..], |v| &v[..]));
        Clean {
            recon: self.planes_of(got, recon),
            pkt: Packet { intra, mv, res: Residual { y: r(0), cb: r(1), cr: r(2) } },
        }
    }
}

/// The vector edit's table for `(swap, scale, rotate)`: every vector of ±`TABLE_RANGE` through `edit`,
/// row-major by y then x, as 32-bit pairs (D3).
pub fn vector_table(edit: impl Fn((i16, i16)) -> (i16, i16)) -> Vec<u8> {
    let r = TABLE_RANGE;
    let mut out = Vec::with_capacity(((2 * r + 1) * (2 * r + 1) * 8) as usize);
    for y in -r..=r {
        for x in -r..=r {
            let v = edit((x as i16, y as i16));
            out.extend_from_slice(&(v.0 as i32).to_le_bytes());
            out.extend_from_slice(&(v.1 as i32).to_le_bytes());
        }
    }
    out
}
