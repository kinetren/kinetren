//! A small lossy audio codec (sim-audio-pairs, notes F-FX-013/F-FX-014): the stream the audio half
//! of a simulated device writes and reads. Modelled on AAC Main, small enough to own:
//!
//! - an MDCT of 2048 samples with a sine window, hop `N` = 1024 (time-domain aliasing cancelled by
//!   overlap-add), computed through a 256-point complex FFT;
//! - per spectral line a backward-adaptive second-order predictor (normalised LMS on the decoder's
//!   own reconstructed spectra, as AAC Main's: no side information) with a RESET flag per frame —
//!   the stream's random-access points;
//! - a quantised residual: one global step (a 1.5 dB ladder) chosen by the rate control, never
//!   finer than the floor the caller sets; a bandwidth by bitrate;
//! - Rice-coded lines in bands of 16 (a 4-bit parameter per band, an escape for large values);
//! - a bit reservoir: constant-rate packet slots, a frame's data may begin in earlier slots
//!   (MPEG-1 Layer III `main_data_begin`), so a lost slot breaks every frame that borrowed from it;
//! - a decoder that checks what it parses (a magnitude past the escape range, a payload that
//!   overruns its length) and CONCEALS a frame it cannot decode: the previous spectrum again, at
//!   −3 dB, its signs scrambled.
//!
//! Everything is a fixed sequence of f64/f32 operations and frozen math (invariant 11): the same
//! stream on every platform. The codec knows nothing of a timeline; the caller drives frames.

use crate::math::FrozenMath;

/// The hop: spectral lines per frame and samples per frame.
pub const N: usize = 1024;
/// The bit-reservoir's capacity (bits a frame may borrow from earlier slots).
pub const RESERVOIR_MAX: usize = 6144;
const BAND: usize = 16;
const ESCAPE: u32 = 20;
/// The band parameter of an all-zero band.
const ZERO_BAND: u32 = 15;
const QMAX: i32 = 8191;
const GAIN_STEPS: i32 = 160;

// ---- the transform ----------------------------------------------------------------------------

/// The fixed tables: the sine window, the DCT-IV pre/post twiddles, the FFT's roots.
struct Tables {
    window: Vec<f64>,
    twiddle: Vec<(f64, f64)>,
    roots: Vec<(f64, f64)>,
}

fn tables() -> &'static Tables {
    static T: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let pi = std::f64::consts::PI;
        let window = (0..2 * N).map(|n| (pi * (n as f64 + 0.5) / (2 * N) as f64).fsin()).collect();
        // DCT-IV of size N through N/2 complex points: z[n] = (r[2n] + i·r[N−1−2n])·e^{−iπ(n+⅛)/N},
        // and the same rotation on the output (the phase's (n + k + ¼)π/N, split in two)
        let twiddle = (0..N / 2)
            .map(|n| {
                let a = -pi * (n as f64 + 0.125) / N as f64;
                (a.fcos(), a.fsin())
            })
            .collect();
        let m = N / 2;
        let roots = (0..m / 2)
            .map(|k| {
                let a = -2.0 * pi * k as f64 / m as f64;
                (a.fcos(), a.fsin())
            })
            .collect();
        Tables { window, twiddle, roots }
    })
}

/// An in-place radix-2 FFT of `re + i·im` (length a power of two).
fn fft(re: &mut [f64], im: &mut [f64], roots: &[(f64, f64)]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = roots[k * step];
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// A complex FFT of a fixed power-of-two length (the roots computed once, by frozen math).
#[derive(Clone)]
pub struct Fft {
    roots: Vec<(f64, f64)>,
    inv: Vec<(f64, f64)>,
}

impl Fft {
    pub fn new(n: usize) -> Fft {
        let roots: Vec<(f64, f64)> = (0..n / 2)
            .map(|k| {
                let a = -2.0 * std::f64::consts::PI * k as f64 / n as f64;
                (a.fcos(), a.fsin())
            })
            .collect();
        let inv = roots.iter().map(|(c, s)| (*c, -*s)).collect();
        Fft { roots, inv }
    }
    /// The forward transform in place (`inverse`: the conjugate one, not scaled).
    pub fn run(&self, re: &mut [f64], im: &mut [f64], inverse: bool) {
        fft(re, im, if inverse { &self.inv } else { &self.roots });
    }
}

/// DCT-IV: `X[k] = Σ r[n]·cos(π/N·(n+½)(k+½))`, through N/2 complex points.
pub fn dct4(r: &[f64], out: &mut [f64]) {
    let t = tables();
    let m = N / 2;
    // stack scratch (local-cost-queue, row 314: two vectors per transform)
    let mut re = [0f64; N / 2];
    let mut im = [0f64; N / 2];
    for n in 0..m {
        let (a, b) = (r[2 * n], r[N - 1 - 2 * n]);
        let (c, s) = t.twiddle[n];
        re[n] = a * c - b * s;
        im[n] = a * s + b * c;
    }
    fft(&mut re, &mut im, &t.roots);
    for k in 0..m {
        let (c, s) = t.twiddle[k];
        let yr = re[k] * c - im[k] * s;
        let yi = re[k] * s + im[k] * c;
        out[2 * k] = yr;
        out[N - 1 - 2 * k] = -yi;
    }
}

/// The MDCT of one windowed block of `2N` samples into `N` lines.
pub fn mdct(x: &[f64], out: &mut [f64]) {
    let w = &tables().window;
    let h = N / 2;
    let mut r = [0f64; N];
    let xw = |n: usize| x[n] * w[n];
    for n in 0..h {
        r[n] = -xw(3 * h + n) - xw(3 * h - 1 - n);
        r[h + n] = xw(n) - xw(N - 1 - n);
    }
    dct4(&r, out);
}

/// The inverse MDCT of `N` lines into a windowed block of `2N` samples (overlap-add two consecutive
/// blocks shifted by `N` to get the signal back).
pub fn imdct(spec: &[f64], out: &mut [f64]) {
    let w = &tables().window;
    let h = N / 2;
    let mut r = [0f64; N];
    dct4(spec, &mut r);
    let s = 2.0 / N as f64;
    for n in 0..h {
        out[3 * h + n] = -r[n] * s;
        out[3 * h - 1 - n] = -r[n] * s;
        out[n] = r[h + n] * s;
        out[N - 1 - n] = -r[h + n] * s;
    }
    for (o, wn) in out.iter_mut().zip(w) {
        *o *= wn;
    }
}

// ---- the predictor ----------------------------------------------------------------------------

/// Per line: the two previous reconstructed values and the two weights (AAC Main's order 2).
#[derive(Clone)]
pub struct Predictor {
    r1: Vec<f64>,
    r2: Vec<f64>,
    a1: Vec<f64>,
    a2: Vec<f64>,
}

const MU: f64 = 0.25;
const LEAK: f64 = 0.96;
const A2_MAX: f64 = 0.9;
const A_MARGIN: f64 = 0.97;
/// The largest spectral line a decoder keeps (twice a full-scale sine's): what a broken stream
/// decodes past it is clipped by the decoder's fixed-point range, as a real decoder's is.
const SPEC_MAX: f64 = N as f64;

impl Predictor {
    pub fn new() -> Predictor {
        Predictor { r1: vec![0.0; N], r2: vec![0.0; N], a1: vec![0.0; N], a2: vec![0.0; N] }
    }
    pub fn reset(&mut self) {
        // in place: a random-access point allocates nothing (row 314)
        for v in [&mut self.r1, &mut self.r2, &mut self.a1, &mut self.a2] {
            v.fill(0.0);
        }
    }
    /// The prediction of every line.
    pub fn predict(&self, out: &mut [f64]) {
        for (k, o) in out.iter_mut().enumerate().take(N) {
            *o = self.a1[k] * self.r1[k] + self.a2[k] * self.r2[k];
        }
    }
    /// Learn from the reconstructed frame `rec` (whose prediction was `pred`).
    pub fn update(&mut self, rec: &[f64], pred: &[f64]) {
        for k in 0..N {
            let e = rec[k] - pred[k];
            let p = self.r1[k] * self.r1[k] + self.r2[k] * self.r2[k] + 1e-9;
            // the weights stay inside the stability triangle (AAC Main's lattice is stable by
            // construction): a decoder fed the wrong history smears, it never runs away
            let a2 = (LEAK * self.a2[k] + MU * e * self.r2[k] / p).clamp(-A2_MAX, A2_MAX);
            let lim = A_MARGIN * (1.0 - a2);
            self.a1[k] = (LEAK * self.a1[k] + MU * e * self.r1[k] / p).clamp(-lim, lim);
            self.a2[k] = a2;
            self.r2[k] = self.r1[k];
            self.r1[k] = rec[k];
        }
    }
}

impl Default for Predictor {
    fn default() -> Predictor {
        Predictor::new()
    }
}

// ---- the entropy code -------------------------------------------------------------------------

/// A bit writer.
#[derive(Clone, Default)]
pub struct Bits {
    pub v: Vec<bool>,
}

impl Bits {
    fn put(&mut self, x: u32, n: u32) {
        for i in (0..n).rev() {
            self.v.push((x >> i) & 1 == 1);
        }
    }
}

fn zig(q: i32) -> u32 {
    if q >= 0 { (q as u32) << 1 } else { (((-q) as u32) << 1) - 1 }
}

fn unzig(u: u32) -> i32 {
    if u & 1 == 0 { (u >> 1) as i32 } else { -(((u + 1) >> 1) as i32) }
}

fn rice_len(u: u32, k: u32) -> usize {
    let q = u >> k;
    if q >= ESCAPE { (ESCAPE + 1 + 14) as usize } else { (q + 1 + k) as usize }
}

/// The best Rice parameter of a band and its cost (with the 4-bit parameter).
fn band_k(us: &[u32]) -> (u32, usize) {
    // an all-zero band is the parameter alone (`ZERO_BAND`)
    if us.iter().all(|u| *u == 0) {
        return (ZERO_BAND, 4);
    }
    let mut best = (0, usize::MAX);
    for k in 0..15 {
        let c: usize = 4 + us.iter().map(|u| rice_len(*u, k)).sum::<usize>();
        if c < best.1 {
            best = (k, c);
        }
    }
    best
}

/// The coded size of `q` (lines `0..bw`) in bits.
pub fn coded_bits(q: &[i32], bw: usize) -> usize {
    let mut total = 0;
    let mut us = [0u32; BAND];
    for b in 0..bw.div_ceil(BAND) {
        let lo = b * BAND;
        let hi = (lo + BAND).min(bw);
        for (i, k) in (lo..hi).enumerate() {
            us[i] = zig(q[k]);
        }
        total += band_k(&us[..hi - lo]).1;
    }
    total
}

/// Code `q` (lines `0..bw`).
pub fn encode_lines(q: &[i32], bw: usize, out: &mut Bits) {
    let mut us = [0u32; BAND];
    for b in 0..bw.div_ceil(BAND) {
        let lo = b * BAND;
        let hi = (lo + BAND).min(bw);
        for (i, k) in (lo..hi).enumerate() {
            us[i] = zig(q[k]);
        }
        let (k, _) = band_k(&us[..hi - lo]);
        out.put(k, 4);
        if k == ZERO_BAND {
            continue;
        }
        for u in &us[..hi - lo] {
            let qq = u >> k;
            if qq >= ESCAPE {
                out.put((1u32 << ESCAPE) - 1, ESCAPE);
                out.put(0, 1);
                out.put(*u, 14);
            } else {
                out.put((1u32 << qq) - 1, qq);
                out.put(0, 1);
                out.put(u & ((1 << k) - 1), k);
            }
        }
    }
}

/// Parse lines `0..bw` from `bits`; `None` — the parse broke (it ran past the payload, or a value
/// left the escape range): the frame is not decodable.
pub fn decode_lines(bits: &[bool], bw: usize, q: &mut [i32]) -> Option<()> {
    let mut p = 0usize;
    let mut get = |n: u32| -> Option<u32> {
        let mut x = 0u32;
        for _ in 0..n {
            x = (x << 1) | *bits.get(p)? as u32;
            p += 1;
        }
        Some(x)
    };
    q.iter_mut().for_each(|v| *v = 0);
    for b in 0..bw.div_ceil(BAND) {
        let lo = b * BAND;
        let hi = (lo + BAND).min(bw);
        let k = get(4)?;
        if k == ZERO_BAND {
            continue;
        }
        for v in q.iter_mut().take(hi).skip(lo) {
            let mut qq = 0;
            while get(1)? == 1 {
                qq += 1;
                if qq > ESCAPE {
                    return None;
                }
            }
            let u = if qq >= ESCAPE { get(14)? } else { (qq << k) | get(k)? };
            let x = unzig(u);
            if x.abs() > QMAX {
                return None;
            }
            *v = x;
        }
    }
    (p == bits.len()).then_some(())
}

// ---- the encoder and the decoder --------------------------------------------------------------

/// The global step of gain index `g` (1.5 dB a step).
pub fn step_of(g: i32) -> f64 {
    2f64.fpowf(g as f64 / 4.0) * 1e-5
}

/// The gain index of a step (the floor's).
pub fn gain_of(step: f64) -> i32 {
    ((step / 1e-5).max(1e-9).flog10() / 2f64.flog10() * 4.0).ceil() as i32
}

/// One coded frame: its gain, its reset flag, the coded lines and the main-data position.
#[derive(Clone)]
pub struct Packet {
    pub gain: i32,
    pub reset: bool,
    pub payload: Bits,
    /// The first bit of this frame's data in the main-data stream (absolute).
    pub begin: usize,
}

/// The encoder of one channel.
#[derive(Clone)]
pub struct Encoder {
    pub pred: Predictor,
    /// The end of the written main data (absolute bits), and the slot size.
    pub written: usize,
    pub slot: usize,
    /// The frames coded so far (the slot index of the next one).
    pub frame: usize,
    spec: Vec<f64>,
    p: Vec<f64>,
    q: Vec<i32>,
    rec: Vec<f64>,
}

impl Encoder {
    /// An encoder at `kbps` for one channel (the slot: `kbps·1000·N/48000` bits).
    pub fn new(kbps: f64) -> Encoder {
        Encoder {
            pred: Predictor::new(),
            written: 0,
            slot: (kbps * 1000.0 * N as f64 / 48_000.0) as usize,
            frame: 0,
            spec: vec![0.0; N],
            p: vec![0.0; N],
            q: vec![0; N],
            rec: vec![0.0; N],
        }
    }

    /// Code the block `x` (2N samples, the frame's MDCT window). `reset` — a random-access point:
    /// the predictor starts over; `floor` — the finest step allowed; `bw` — the coded lines;
    /// `predict` — use the predictor at all (off: every frame is self-contained).
    pub fn encode(&mut self, x: &[f64], reset: bool, floor: f64, bw: usize, predict: bool) -> Packet {
        mdct(x, &mut self.spec);
        if reset {
            self.pred.reset();
        }
        if predict {
            self.pred.predict(&mut self.p);
        } else {
            self.p.iter_mut().for_each(|v| *v = 0.0);
        }
        // the main data may begin up to RESERVOIR_MAX bits back, never before what is written
        let end_of_slot = (self.frame + 1) * self.slot;
        let begin = self.written.max(end_of_slot.saturating_sub(self.slot + RESERVOIR_MAX));
        let avail = end_of_slot.saturating_sub(begin);
        // the rate control: the finest gain at or above the floor that fits (a bisection: the coded
        // size falls as the step grows)
        let quant = |g: i32, q: &mut [i32], spec: &[f64], p: &[f64]| {
            let step = step_of(g);
            for k in 0..N {
                q[k] = if k < bw { ((spec[k] - p[k]) / step).round().clamp(-(QMAX as f64), QMAX as f64) as i32 } else { 0 };
            }
        };
        let g0 = gain_of(floor).clamp(0, GAIN_STEPS);
        quant(g0, &mut self.q, &self.spec, &self.p);
        let mut g = g0;
        // the coded size of the gain chosen, known from the search where it measured it: the payload
        // is allocated once at its size (row 314)
        let mut size = coded_bits(&self.q, bw);
        if size > avail {
            let (mut lo, mut hi, mut hi_size) = (g0, GAIN_STEPS, None);
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                quant(mid, &mut self.q, &self.spec, &self.p);
                let c = coded_bits(&self.q, bw);
                if c <= avail {
                    hi = mid;
                    hi_size = Some(c);
                } else {
                    lo = mid;
                }
            }
            g = hi;
            quant(g, &mut self.q, &self.spec, &self.p);
            size = hi_size.unwrap_or_else(|| coded_bits(&self.q, bw));
        }
        let step = step_of(g);
        for k in 0..N {
            self.rec[k] = self.p[k] + self.q[k] as f64 * step;
        }
        if predict {
            self.pred.update(&self.rec, &self.p);
        }
        let mut payload = Bits { v: Vec::with_capacity(size) };
        encode_lines(&self.q, bw, &mut payload);
        debug_assert_eq!(payload.v.len(), size, "coded_bits is encode_lines' size");
        self.written = begin + payload.v.len();
        self.frame += 1;
        Packet { gain: g, reset, payload, begin }
    }
}

/// What the decoder is told about a frame besides its bits.
#[derive(Clone, Copy, Default)]
pub struct Damage {
    /// The packet (or a slot its data lives in) is lost: the frame cannot be decoded.
    pub lost: bool,
    /// The reset flag did not arrive with it (the random-access point was deleted).
    pub no_reset: bool,
    /// The reference stops updating (the predictor's history is held).
    pub freeze: bool,
    /// The share of the decoded residual applied (1 — all).
    pub residual: f64,
}

/// The decoder of one channel.
#[derive(Clone)]
pub struct Decoder {
    pub pred: Predictor,
    last: Vec<f64>,
    spec: Vec<f64>,
    p: Vec<f64>,
    q: Vec<i32>,
    block: Vec<f64>,
    /// The second half of the previous block (overlap-add).
    tail: Vec<f64>,
    /// Frames concealed so far.
    pub concealed: u64,
    seed: u32,
}

impl Decoder {
    pub fn new(seed: u32) -> Decoder {
        Decoder {
            pred: Predictor::new(),
            last: vec![0.0; N],
            spec: vec![0.0; N],
            p: vec![0.0; N],
            q: vec![0; N],
            block: vec![0.0; 2 * N],
            tail: vec![0.0; N],
            concealed: 0,
            seed,
        }
    }

    /// Decode `pkt` (its payload as received: `bits`) into `N` output samples.
    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, pkt: &Packet, bits: &[bool], dmg: Damage, bw: usize, predict: bool, frame: u64, out: &mut [f64]) {
        if pkt.reset && !dmg.no_reset && !dmg.lost {
            self.pred.reset();
        }
        if predict {
            self.pred.predict(&mut self.p);
        } else {
            self.p.iter_mut().for_each(|v| *v = 0.0);
        }
        let ok = !dmg.lost && decode_lines(bits, bw, &mut self.q).is_some();
        if ok {
            let step = step_of(pkt.gain);
            for k in 0..N {
                self.spec[k] = (self.p[k] + self.q[k] as f64 * step * dmg.residual).clamp(-SPEC_MAX, SPEC_MAX);
            }
        } else {
            // concealment: the last spectrum again at −3 dB, signs scrambled by a hash of the frame
            self.concealed += 1;
            for k in 0..N {
                let h = crate::generator::lowbias32(self.seed ^ (frame as u32).wrapping_mul(0x9E37_79B9) ^ k as u32);
                let s = if h & 1 == 0 { 1.0 } else { -1.0 };
                self.spec[k] = self.last[k] * 0.708 * s;
            }
        }
        if predict && !dmg.freeze {
            self.pred.update(&self.spec, &self.p);
        }
        self.last.copy_from_slice(&self.spec);
        imdct(&self.spec, &mut self.block);
        for (n, o) in out.iter_mut().enumerate().take(N) {
            *o = self.tail[n] + self.block[n];
            self.tail[n] = self.block[N + n];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1470_dct4_matches_the_sum() {
        let r: Vec<f64> = (0..N).map(|n| ((n * 37 % 101) as f64 - 50.0) / 50.0).collect();
        let mut fast = vec![0.0; N];
        dct4(&r, &mut fast);
        let pi = std::f64::consts::PI;
        for k in [0usize, 1, 7, 300, 511, 512, 1000, 1023] {
            let mut s = 0.0;
            for (n, x) in r.iter().enumerate() {
                s += x * (pi / N as f64 * (n as f64 + 0.5) * (k as f64 + 0.5)).fcos();
            }
            assert!((s - fast[k]).abs() < 1e-8 * N as f64, "k {k}: {s} vs {}", fast[k]);
        }
    }

    #[test]
    fn t1470b_mdct_overlap_add_is_perfect() {
        let sig: Vec<f64> = (0..6 * N).map(|n| ((n as f64 * 0.0123).fsin() + 0.3 * (n as f64 * 0.31).fcos()) * 0.5).collect();
        let mut out = vec![0f64; 6 * N];
        let mut tail = vec![0f64; N];
        let mut spec = vec![0f64; N];
        let mut block = vec![0f64; 2 * N];
        // frame f covers [(f−1)N, (f+1)N)
        for f in 1..5 {
            mdct(&sig[(f - 1) * N..(f + 1) * N], &mut spec);
            imdct(&spec, &mut block);
            for n in 0..N {
                out[(f - 1) * N + n] = tail[n] + block[n];
                tail[n] = block[N + n];
            }
        }
        for n in N..4 * N {
            assert!((out[n] - sig[n]).abs() < 1e-9, "sample {n}: {} vs {}", out[n], sig[n]);
        }
    }

    #[test]
    fn t1470c_rice_round_trip() {
        let q: Vec<i32> = (0..N).map(|k| if k % 3 == 0 { (k as i32 % 41) - 20 } else { 0 }).collect();
        let mut b = Bits::default();
        encode_lines(&q, 700, &mut b);
        assert_eq!(b.v.len(), coded_bits(&q, 700));
        let mut back = vec![0; N];
        decode_lines(&b.v, 700, &mut back).expect("parses");
        assert_eq!(&back[..700], &q[..700]);
    }
}
