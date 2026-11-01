//! The deck: the passport's physical quantities, the parameter words both executors read (computed
//! once per frame in f64 and rounded to f32 ONCE, design D7/D8), and the line table — per row: which
//! head reads which track, the head-to-track coverage, the gains, the noise, the dropout, the time-base
//! error and the row's two channel kernels (design D3). `line_row` is mirrored by `wgsl/lt.wgsl`.

use kt_plugin_api::barrier::noise_seed;
use kt_plugin_api::math::FrozenMath;

use crate::sig::{atan2_rad, cos_cyc, gauss, hash, hash3, sin_cyc, unit};
use crate::standard::{Standard, Std, KN, KR};

/// The passport rows, in file order.
pub mod row {
    pub const STANDARD: usize = 0;
    pub const SPEED: usize = 1;
    pub const DECKS: usize = 2;
    pub const HEADS: usize = 3;
    pub const TRANSPORT: usize = 4;
    pub const TBC: usize = 5;
    pub const DOC: usize = 6;
    pub const COMB: usize = 7;
    /// Head rows: drum 1 (records; plays too with one deck) A, B; drum 2 (plays, two decks) A, B —
    /// each spacing µm, gap µm, output dB.
    pub const HEAD0: usize = 8;
    pub const TRACKING: usize = 20;
    pub const SKEW: usize = 21;
    pub const CN: usize = 22;
    pub const DROPOUTS: usize = 23;
    pub const DROPOUT_LEN: usize = 24;
    pub const JITTER: usize = 25;
    pub const SWITCH_POINT: usize = 26;
    pub const SWITCH_SKEW: usize = 27;
    pub const WHITE_CLIP: usize = 28;
    pub const DARK_CLIP: usize = 29;
    pub const CHROMA_LEVEL: usize = 30;
    pub const SATURATION: usize = 31;
    pub const COUNT: usize = 32;

    /// The first row of drum `d` (0 record, 1 play), head `h` (0 A, 1 B).
    pub const fn head(d: usize, h: usize) -> usize {
        HEAD0 + (d * 2 + h) * 3
    }
}

/// Words of the parameter texture (48 = three upload passes of 16).
pub const WORDS: usize = 48;
/// Columns of the line table: 16 scalars, then the own and the neighbour kernels.
pub const LT_SCALARS: usize = 16;
pub const LT_W: usize = LT_SCALARS + 2 * KN;

/// Word indices. A word is an integer or the bits of an f32; the upload passes carry words 0–7 of
/// each block of 16 in their u32 uniforms, so every INTEGER word sits there (an arbitrary u32 in an
/// f32 uniform could be a NaN the driver rewrites).
pub mod w {
    // block 0 (u slots 0..7: integers)
    pub const STD: usize = 0;
    pub const H: usize = 1;
    pub const W: usize = 2;
    pub const FLAGS: usize = 3;
    pub const FRAME: usize = 4;
    pub const SEED: usize = 5;
    pub const V_SWITCH: usize = 6;
    pub const M: usize = 7;
    pub const PITCH: usize = 8;
    pub const HEAD_W: usize = 9;
    pub const TRACKING: usize = 10;
    pub const SKEW: usize = 11;
    pub const CROSSING: usize = 12;
    pub const AZ_PER_UNIT: usize = 13;
    pub const VAR_TAPE: usize = 14;
    pub const VAR_AMP: usize = 15;
    // block 1 (word 16 an integer)
    pub const A: usize = 16;
    pub const P_DROP: usize = 17;
    pub const DROP_LEN: usize = 18;
    pub const JITTER: usize = 19;
    pub const SWITCH_POINT: usize = 20;
    pub const SWITCH_SKEW: usize = 21;
    pub const PH_DRUM: usize = 22;
    pub const PH_FLUT: usize = 23;
    pub const DRUM_LINE: usize = 24;
    pub const FLUT_LINE: usize = 25;
    pub const DRUM_FIELD: usize = 26;
    pub const FLUT_FIELD: usize = 27;
    pub const FCU_N: usize = 28;
    pub const A_C: usize = 29;
    pub const ROT_A: usize = 30;
    pub const ROT_B: usize = 31;
    /// block 2: drum d, head h: spacing (samples), gap (samples), gain — 32 + (2d + h)·3
    pub const HEADS: usize = 32;
    pub const BURST: usize = 44;
}

pub const F_TBC: u32 = 1;
pub const F_DOC: u32 = 2;
pub const F_COMB: u32 = 4;
pub const F_STILL4: u32 = 8;

/// The flutter rate of the capstan (Hz) — one fixed component of the time-base error.
const FLUTTER_HZ: f64 = 5.3;
/// The share of the noise power the tape gives at the reference width (the rest is the amplifier).
const TAPE_SHARE: f64 = 0.7;
/// The noise of the switching transient (RF units).
const BURST_SIGMA: f64 = 0.9;
/// tan 12°: two ±6° gaps against each other.
const TAN12: f64 = 0.212_556_56;

/// The deck as read from the passport values.
#[derive(Debug, Clone)]
pub struct Deck {
    pub st: &'static Standard,
    pub v: Vec<f64>,
}

impl Deck {
    pub fn new(values: &[f64]) -> Deck {
        let mut v = crate::defaults();
        for (o, x) in v.iter_mut().zip(values) {
            *o = *x;
        }
        let std = if v[row::STANDARD].round() as i64 == 1 { Std::Ntsc } else { Std::Pal };
        Deck { st: Standard::get(std), v }
    }

    fn flag(&self, r: usize) -> bool {
        self.v[r] >= 0.5
    }
    fn pick(&self, r: usize) -> u32 {
        self.v[r].round().max(0.0) as u32
    }
    pub fn tbc(&self) -> bool {
        self.flag(row::TBC)
    }
    pub fn comb(&self) -> bool {
        self.flag(row::COMB)
    }
    pub fn doc(&self) -> bool {
        self.flag(row::DOC)
    }
    pub fn two_decks(&self) -> bool {
        self.pick(row::DECKS) == 1
    }
    pub fn four_heads(&self) -> bool {
        self.pick(row::HEADS) == 1
    }
    pub fn still(&self) -> bool {
        self.pick(row::TRANSPORT) == 1
    }
    /// The FM chroma record level (linear).
    pub fn a_c(&self) -> f64 {
        10f64.fpowf(self.v[row::CHROMA_LEVEL].clamp(-40.0, 0.0) / 20.0)
    }

    /// The parameter words of the frame at `burst_index` (ticks): f64 here, rounded once to f32.
    pub fn words(&self, burst_index: i64) -> [u32; WORDS] {
        let st = self.st;
        let v = &self.v;
        let mut o = [0u32; WORDS];
        let f = |x: f64| (x as f32).to_bits();
        let t = burst_index as f64 / 90_000.0;
        let frame = (t * st.frame_rate).floor();
        o[w::STD] = (st.std == Std::Ntsc) as u32;
        o[w::H] = st.h;
        o[w::W] = st.w;
        let still4 = self.still() && self.four_heads();
        o[w::FLAGS] = (self.tbc() as u32 * F_TBC)
            | (self.doc() as u32 * F_DOC)
            | (self.comb() as u32 * F_COMB)
            | (still4 as u32 * F_STILL4);
        o[w::FRAME] = (frame as i64) as u32;
        o[w::SEED] = noise_seed(burst_index);
        // geometry in units of the SP pitch (the reference signal width)
        let speed = self.pick(row::SPEED).min(2);
        let pitch = 1.0 / (1 + speed) as f64;
        let head_w = match (self.four_heads(), speed) {
            // a two-head drum: one compromise pair for every speed
            (false, _) => 0.65,
            (true, 0) => 1.0,
            // the four-head drum's narrow pair, used at LP and EP
            (true, _) => 1.15 * 0.5,
        };
        let um = 1e-6 / st.pitch_sp;
        o[w::PITCH] = f(pitch);
        o[w::HEAD_W] = f(head_w);
        o[w::TRACKING] = f(v[row::TRACKING] * um);
        o[w::SKEW] = f(v[row::SKEW] * um);
        // a stopped tape: the two-head scan starts on the track and leaves it by one pitch over the
        // field; four heads hold the field
        o[w::CROSSING] = f(if self.still() && !self.four_heads() { pitch } else { 0.0 });
        // samples of time per µm along the track = fs / v_write · 1e-6
        let per_um = st.fs / st.v_write * 1e-6;
        o[w::AZ_PER_UNIT] = f(st.pitch_sp * 1e6 * per_um * TAN12);
        // C/N at the reference width: noise power against the carrier's 1/2
        let n = 0.5 * 10f64.fpowf(-v[row::CN] / 10.0);
        o[w::VAR_TAPE] = f(TAPE_SHARE * n);
        o[w::VAR_AMP] = f((1.0 - TAPE_SHARE) * n);
        o[w::P_DROP] = f((v[row::DROPOUTS].max(0.0) / (st.h as f64 * st.frame_rate)).min(1.0));
        o[w::DROP_LEN] = f(v[row::DROPOUT_LEN].max(0.0) * st.per_us());
        o[w::JITTER] = f(v[row::JITTER].max(0.0) * st.per_us());
        o[w::SWITCH_POINT] = f(v[row::SWITCH_POINT]);
        o[w::SWITCH_SKEW] = f(v[row::SWITCH_SKEW] * st.per_us());
        // the slow parts of the time-base error: phases at the frame start, then advances per line
        // and per field (f64 from the absolute time — f32 would lose the cycle after minutes)
        let frac = |x: f64| x - x.floor();
        let t0 = frame / st.frame_rate;
        o[w::PH_DRUM] = f(frac(st.drum_rate * t0));
        o[w::PH_FLUT] = f(frac(FLUTTER_HZ * t0 + 0.37));
        o[w::DRUM_LINE] = f(st.drum_rate * st.t_line);
        o[w::FLUT_LINE] = f(FLUTTER_HZ * st.t_line);
        let t_field = 0.5 / st.frame_rate;
        o[w::DRUM_FIELD] = f(st.drum_rate * t_field);
        o[w::FLUT_FIELD] = f(FLUTTER_HZ * t_field);
        o[w::FCU_N] = f(st.fcu / st.fs);
        o[w::A_C] = f(self.a_c());
        o[w::ROT_A] = f(st.rot[0] as f64);
        o[w::ROT_B] = f(st.rot[1] as f64);
        o[w::V_SWITCH] = st.v_switch as u32;
        o[w::M] = st.m;
        o[w::A] = st.a;
        for d in 0..2 {
            for h in 0..2 {
                // one deck: the same drum records and plays
                let r = row::head(if self.two_decks() { d } else { 0 }, h);
                let i = w::HEADS + (2 * d + h) * 3;
                o[i] = f(v[r].max(0.0) * per_um);
                o[i + 1] = f(v[r + 1].max(0.01) * per_um);
                o[i + 2] = f(10f64.fpowf(v[r + 2].min(12.0) / 20.0));
            }
        }
        o[w::BURST] = f(BURST_SIGMA);
        o
    }
}

/// An f32 word.
#[inline]
pub fn wf(c: &[u32; WORDS], i: usize) -> f32 {
    f32::from_bits(c[i])
}

/// The overlap of the head [lo_h, hi_h] with [lo, hi].
#[inline]
fn ov(lo: f32, hi: f32, lo_h: f32, hi_h: f32) -> f32 {
    (hi.min(hi_h) - lo.max(lo_h)).max(0.0)
}

/// J(t) = ∫ C(t) dt, C(t) = atan(t/a)/π the Lorentzian's CDF (less ½): (t·atan(t/a) − ½a·ln(a² + t²))/π.
#[inline]
fn lor_j(t: f32, a: f32) -> f32 {
    (t * atan2_rad(t, a) - 0.5 * a * ((a * a + t * t).flog10() * std::f32::consts::LN_10)) * (1.0 / std::f32::consts::PI)
}

/// One tap of a channel kernel: the exact bin integral over [k − ½, k + ½] of a Lorentzian of
/// half-width `a` (spacing loss) convolved with a unit-area box of `b` samples (gap, plus the
/// azimuth strip for the neighbour) — a second difference of `lor_j` (design D3). Its tails are
/// absolute errors of ~1e-6, harmless in a sum.
#[inline]
pub fn tap(k: i32, a: f32, b: f32) -> f32 {
    let t = k as f32;
    let hb = 0.5 * b;
    ((lor_j(t + 0.5 + hb, a) - lor_j(t - 0.5 + hb, a)) - (lor_j(t + 0.5 - hb, a) - lor_j(t - 0.5 - hb, a))) / b
}

/// The geometry of a row: (content row, playing head, the track's record head, switched).
#[derive(Debug, Clone, Copy)]
pub struct RowGeo {
    pub content: u32,
    pub play: u32,
    pub rec: u32,
    pub switched: bool,
    pub yf: u32,
    pub field: u32,
    /// coverages (units of the SP pitch): own, the other-azimuth neighbours, the widest neighbour
    /// strip, the next-but-one
    pub c0: f32,
    pub c1: f32,
    pub c1w: f32,
    pub c2: f32,
}

pub fn row_geo(c: &[u32; WORDS], y: u32) -> RowGeo {
    let hf = c[w::H] / 2;
    let field = y & 1;
    let yf = y >> 1;
    let still4 = c[w::FLAGS] & F_STILL4 != 0;
    let sp = wf(c, w::SWITCH_POINT);
    let switched = (yf as f32 + 0.5) > (hf as f32 - sp);
    let content = if still4 && field == 1 { y - 1 } else { y };
    let head = if still4 {
        0
    } else if switched {
        1 - field
    } else {
        field
    };
    let mut u = (yf as f32 + 0.5) / hf as f32;
    if switched {
        // the other head is at the START of the next track
        u -= 1.0;
    }
    let p = wf(c, w::PITCH);
    let o = wf(c, w::TRACKING) + wf(c, w::SKEW) * (u - 0.5) + wf(c, w::CROSSING) * u;
    let hw = wf(c, w::HEAD_W);
    let (lo, hi) = (o - 0.5 * hw, o + 0.5 * hw);
    let c0 = ov(-0.5 * p, 0.5 * p, lo, hi);
    let c1a = ov(0.5 * p, 1.5 * p, lo, hi);
    let c1b = ov(-1.5 * p, -0.5 * p, lo, hi);
    let c2 = ov(1.5 * p, 2.5 * p, lo, hi) + ov(-2.5 * p, -1.5 * p, lo, hi);
    RowGeo { content, play: head, rec: head, switched, yf, field, c0, c1: c1a + c1b, c1w: c1a.max(c1b), c2 }
}

/// Spacing, gap and gain of drum `d`, head `h`.
#[inline]
fn head(c: &[u32; WORDS], d: u32, h: u32) -> (f32, f32, f32) {
    let i = w::HEADS + (2 * d + h) as usize * 3;
    (wf(c, i), wf(c, i + 1), wf(c, i + 2))
}

/// The kernel parameters (a, b) of a row's own track and of its neighbour.
pub fn kernel_ab(c: &[u32; WORDS], g: &RowGeo) -> [(f32, f32); 2] {
    let pd = 1;
    let (ps, pg, _) = head(c, pd, g.play);
    let (rs, _, _) = head(c, 0, g.rec);
    let (xs, _, _) = head(c, 0, 1 - g.rec);
    let own = ((rs + ps).max(0.001), pg.max(0.05));
    let cross = ((xs + ps).max(0.001), (pg + g.c1w * wf(c, w::AZ_PER_UNIT)).max(0.05));
    [own, cross]
}

/// Column indices of the line table's scalars.
pub mod lt {
    pub const CONTENT: usize = 0;
    pub const ADJ: usize = 1;
    pub const G_OWN: usize = 2;
    pub const G_X: usize = 3;
    pub const SIGMA: usize = 4;
    pub const DROP_START: usize = 5;
    pub const DROP_LEN: usize = 6;
    pub const TBE: usize = 7;
    pub const BURST: usize = 8;
    pub const ACC: usize = 9;
    pub const ROT: usize = 10;
    pub const VSIGN: usize = 11;
    pub const YF: usize = 12;
}

/// One scalar column `x` (< LT_SCALARS) of row `y`. A function per column, so the GPU texel computes
/// exactly its own (`wgsl/lt.wgsl`).
pub fn lt_scalar(c: &[u32; WORDS], y: u32, x: usize) -> f32 {
    let g = row_geo(c, y);
    let (_, _, pgain) = head(c, 1, g.play);
    let (_, _, rgain) = head(c, 0, g.rec);
    let (_, _, xgain) = head(c, 0, 1 - g.rec);
    let frame = c[w::FRAME];
    match x {
        lt::CONTENT => g.content as f32,
        lt::ADJ => (g.content ^ 1) as f32,
        lt::G_OWN => pgain * rgain * (g.c0 + g.c2),
        lt::G_X => pgain * xgain * g.c1,
        lt::SIGMA => (wf(c, w::VAR_TAPE) * (g.c0 + g.c1 + g.c2) * (pgain * pgain) + wf(c, w::VAR_AMP)).sqrt(),
        lt::DROP_START | lt::DROP_LEN => {
            let h = hash3(frame, y, 0x0D0D);
            if unit(h) < wf(c, w::P_DROP) {
                if x == lt::DROP_START {
                    unit(hash(h ^ 1)) * c[w::W] as f32
                } else {
                    wf(c, w::DROP_LEN) * (0.5 + unit(hash(h ^ 2)))
                }
            } else {
                -1.0
            }
        }
        lt::TBE => {
            let mut e = 0.0f32;
            if c[w::FLAGS] & F_TBC == 0 {
                let fl = g.field as f32;
                let yf = g.yf as f32;
                let drum = wf(c, w::PH_DRUM) + wf(c, w::DRUM_FIELD) * fl + wf(c, w::DRUM_LINE) * yf;
                let flut = wf(c, w::PH_FLUT) + wf(c, w::FLUT_FIELD) * fl + wf(c, w::FLUT_LINE) * yf;
                let rnd = gauss(hash3(frame, y, 0x7BE));
                e = wf(c, w::JITTER) * (0.6 * sin_cyc(drum) + 0.5 * sin_cyc(flut) + 0.6 * rnd);
                if g.switched {
                    e += wf(c, w::SWITCH_SKEW);
                }
            }
            e
        }
        lt::BURST => {
            let hf = (c[w::H] / 2) as f32;
            let at = hf - wf(c, w::SWITCH_POINT);
            if g.yf as f32 == at.floor() { (at - at.floor()) * c[w::W] as f32 } else { -1.0 }
        }
        lt::ACC => {
            // the chroma's channel gain at the colour-under: the own kernel's transfer at fcu
            let [(a, b), _] = kernel_ab(c, &g);
            let fcu = wf(c, w::FCU_N);
            let mut t = 0.0f32;
            for k in 0..KN as i32 {
                t += tap(k - KR as i32, a, b) * cos_cyc(fcu * (k - KR as i32) as f32);
            }
            let gc = (pgain * rgain * (g.c0 + g.c2) * t).max(1.0 / 6.0);
            1.0 / (gc * wf(c, w::A_C))
        }
        lt::ROT => {
            if g.rec == 0 { wf(c, w::ROT_A) } else { wf(c, w::ROT_B) }
        }
        lt::VSIGN => {
            if c[w::V_SWITCH] != 0 && (g.content >> 1) & 1 == 1 { -1.0 } else { 1.0 }
        }
        lt::YF => (g.content >> 1) as f32,
        _ => 0.0,
    }
}

/// A whole line-table row: the scalars, then the own kernel's and the neighbour kernel's taps.
pub fn line_row(c: &[u32; WORDS], y: u32, out: &mut [f32]) {
    for (x, o) in out[..LT_SCALARS].iter_mut().enumerate() {
        *o = lt_scalar(c, y, x);
    }
    let g = row_geo(c, y);
    let [(a, b), (ax, bx)] = kernel_ab(c, &g);
    for k in 0..KN {
        out[LT_SCALARS + k] = tap(k as i32 - KR as i32, a, b);
        out[LT_SCALARS + KN + k] = tap(k as i32 - KR as i32, ax, bx);
    }
}
