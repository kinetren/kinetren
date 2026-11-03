//! The sound of the tape (sim-audio-pairs; note F-FX-012): the audio half of `vhs_tape`. The same
//! deck plays the sound the picture was recorded with, through the two VHS audio systems:
//!
//! - **the linear track** — a STATIONARY head along the tape edge at the tape's own speed: the
//!   record level and the bias set the tape's transfer curve, the gap, spacing and coating
//!   thickness losses at the wavelength `v_tape / f` cut the top (LP is dull because its wavelength
//!   is half SP's), tape hiss grows as the track narrows (mono 1.0 mm, stereo 0.35 mm), the oxide
//!   defects of the picture's rate pass the head as dropouts as long as the defect's length over
//!   the tape speed, the capstan's wow and flutter is the picture's 5.3 Hz component, Dolby B if set;
//! - **Hi-Fi** — FM carriers (PAL 1.4 / 1.8 MHz, NTSC 1.3 / 1.7 MHz, ±150 kHz) written by heads on
//!   the drum: a 2:1 compander and an emphasis around them, the carrier read by the head that plays
//!   the picture's line at that moment (row geometry, spacing and gap losses at the carrier's
//!   wavelength, the head's output, the tracking coverage), the picture's own dropout field cutting
//!   the carrier, complex FM noise from the tape's C/N, a demodulator that breaks into clicks below
//!   the FM threshold, the deck's switching-pulse hold and dropout hold (the DOC switch).
//!
//! The DEVICE STATE is never shared with the picture: both halves derive it from the same rows and
//! the same TIMELINE time (`Deck::words`, `deck::row_geo`, `deck::lt_scalar` are the picture's own
//! functions), so a field the picture loses is the field the sound loses (invariant 2).
//! Everything random is a hash of the timeline sample — a seek gives the same noise (invariant 11).

use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{AudioState, Placement};

use crate::deck::{self, row, Deck, WORDS};
use crate::sig::{atan2_cyc, cos_cyc, gauss, hash, hash3, sin_cyc, unit};
use crate::standard::Std;

/// The audio look-ahead (frames): the flutter's delay line is centred here, the linear track's loss
/// filter takes `LIN_M` of it.
pub const LA: i64 = 96;
/// The half-length of the linear track's loss filter (65 taps).
const LIN_M: usize = 32;
/// The Hi-Fi FM simulation's oversampling of the audio rate (384 kHz complex baseband).
pub const OS: usize = 8;
const FS: f64 = 48_000.0;
/// The Hi-Fi deviation (Hz).
const DEV: f64 = 150_000.0;
/// The capstan's flutter rate (Hz) and phase — `deck`'s time-base component (`FLUTTER_HZ`, 0.37).
const FLUTTER_HZ: f64 = 5.3;
const FLUTTER_PH: f64 = 0.37;
/// The linear track's displacement per µs of the deck's jitter row (samples): the jitter row is
/// the deck's mechanical quality; 0.05 µs (the default) gives a 0.15 % peak capstan flutter.
const CAP_SAMPLES_PER_US: f64 = 43.2;
/// Scrape flutter of the stationary head: a random displacement on a 5 ms grid, per µs of jitter.
const SCRAPE_SAMPLES_PER_US: f64 = 2.0;
const SCRAPE_CELL: f64 = 0.005;
/// The recording reference: −12 dBFS is the tape's reference flux.
const REF: f32 = 0.25;
/// The coating thickness (m) and the deck's nominal stationary head (gap, spacing; m) its playback
/// EQ was designed for at SP.
const COATING: f64 = 4.0e-6;
const NOM_GAP: f64 = 1.0e-6;
const NOM_SPACING: f64 = 0.1e-6;
/// The playback EQ's ceiling (×10 = +20 dB).
const EQ_MAX: f64 = 10.0;
/// The linear track's SNR at SP mono, the tape's reference C/N (dB).
const LIN_SNR_DB: f64 = 42.0;
const CN_REF_DB: f64 = 43.0;
/// The Hi-Fi C/N below the video FM's (the deep layer, a narrower band per channel).
const HIFI_CN_BELOW: f64 = 8.0;
/// A 2-head drum's own-track coverage at SP: the Hi-Fi carrier's reference.
const C0_REF: f32 = 0.65;
/// The carrier detector: below this C/N (power ratio) the deck holds (with DOC on).
const CARRIER_DETECT: f32 = 4.0;
/// Dolby B: the sliding band's corner (Hz), its full boost (1 + g = +10 dB) and knee level.
const DOLBY_HZ: f64 = 1500.0;
const DOLBY_G: f32 = 2.162_278;
const DOLBY_E0: f32 = 0.01;

/// The tape speed (m/s) of the linear track: SP, LP, EP.
pub fn tape_speed(std: Std, speed: u32) -> f64 {
    let sp = match std {
        Std::Pal => 0.023_39,
        Std::Ntsc => 0.033_35,
    };
    sp / (1 + speed.min(2)) as f64
}

/// The Hi-Fi carriers (Hz) of the left and right channels.
pub fn hifi_carriers(std: Std) -> [f64; 2] {
    match std {
        Std::Pal => [1.4e6, 1.8e6],
        Std::Ntsc => [1.3e6, 1.7e6],
    }
}

/// The track widths of the linear audio (m): one mono track, or two stereo tracks.
pub fn linear_width(stereo: bool) -> f64 {
    if stereo { 0.35e-3 } else { 1.0e-3 }
}

/// Wallace's spacing loss (amplitude) at wavelength `lambda`.
#[inline]
fn spacing_loss(d: f64, lambda: f64) -> f64 {
    (-std::f64::consts::TAU * d / lambda).fexp()
}

/// The gap loss sin(πg/λ)/(πg/λ) — signed: past its first null the response inverts.
#[inline]
fn gap_loss(g: f64, lambda: f64) -> f64 {
    let x = std::f64::consts::PI * g / lambda;
    if x < 1e-9 { 1.0 } else { x.fsin() / x }
}

/// The thickness loss of a coating `delta` deep.
#[inline]
fn thickness_loss(delta: f64, lambda: f64) -> f64 {
    let k = std::f64::consts::TAU * delta / lambda;
    if k < 1e-9 { 1.0 } else { (1.0 - (-k).fexp()) / k }
}

/// The linear track's response at `f` Hz: the tape and head losses at this speed, then the deck's
/// playback EQ — the inverse of the NOMINAL head at SP, capped at +20 dB. A clean head at SP is
/// flat to where the cap bites; LP, a worn gap or a dirty head lose the top.
pub fn linear_response(f: f64, v: f64, v_sp: f64, gap: f64, spacing: f64) -> f64 {
    if f <= 0.0 {
        return 1.0;
    }
    let lam = v / f;
    let lam_sp = v_sp / f;
    let actual = gap_loss(gap, lam) * spacing_loss(spacing, lam) * thickness_loss(COATING, lam);
    let nominal = gap_loss(NOM_GAP, lam_sp) * spacing_loss(NOM_SPACING, lam_sp) * thickness_loss(COATING, lam_sp);
    let eq = if nominal.abs() < 1.0 / EQ_MAX { EQ_MAX } else { 1.0 / nominal.abs() };
    actual * eq
}

/// The 65-tap zero-phase FIR of `linear_response` (the response integrated over 256 bands, a Hann
/// window), designed once per configuration. Frozen cosines: the same taps on every platform.
pub fn design_linear(v: f64, v_sp: f64, gap: f64, spacing: f64) -> [f32; 2 * LIN_M + 1] {
    const K: usize = 256;
    let mut resp = [0f64; K];
    for (j, r) in resp.iter_mut().enumerate() {
        let f = (j as f64 + 0.5) / K as f64 * FS * 0.5;
        *r = linear_response(f, v, v_sp, gap, spacing);
    }
    let mut h = [0f32; 2 * LIN_M + 1];
    for k in 0..=LIN_M {
        let mut acc = 0.0;
        for (j, r) in resp.iter().enumerate() {
            acc += r * (std::f64::consts::PI * (j as f64 + 0.5) * k as f64 / K as f64).fcos();
        }
        let w = 0.5 + 0.5 * (std::f64::consts::PI * k as f64 / (LIN_M as f64 + 1.0)).fcos();
        let v = (acc / K as f64 * w) as f32;
        h[LIN_M + k] = v;
        h[LIN_M - k] = v;
    }
    h
}

/// A one-pole lowpass coefficient for corner `hz`.
fn one_pole(hz: f64) -> f32 {
    (1.0 - (-std::f64::consts::TAU * hz / FS).fexp()) as f32
}

/// tanh by the frozen exp.
#[inline]
fn tanh(x: f32) -> f32 {
    let e = (2.0 * x.clamp(-15.0, 15.0)).fexp();
    (e - 1.0) / (e + 1.0)
}

/// A first-order emphasis shelf (zero 2 kHz, pole 8 kHz, +12 dB at the top) and its exact inverse.
#[derive(Clone, Copy, Default)]
struct Shelf {
    x1: f32,
    y1: f32,
}

/// The bilinear shelf's coefficients (b0, b1, a1) of H(s) = (1 + s/wz) / (1 + s/wp) · wp/wz⁻¹ (unit DC).
fn shelf_coefs() -> (f32, f32, f32) {
    let k = 2.0 * FS;
    let (wz, wp) = (std::f64::consts::TAU * 2000.0, std::f64::consts::TAU * 8000.0);
    // H(s) = (s/wz + 1) / (s/wp + 1); bilinear s = k (1 − z⁻¹)/(1 + z⁻¹)
    let (n0, n1) = (k / wz + 1.0, 1.0 - k / wz);
    let (d0, d1) = (k / wp + 1.0, 1.0 - k / wp);
    ((n0 / d0) as f32, (n1 / d0) as f32, (d1 / d0) as f32)
}

impl Shelf {
    fn pre(&mut self, x: f32, c: (f32, f32, f32)) -> f32 {
        let y = c.0 * x + c.1 * self.x1 - c.2 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
    /// The exact inverse: `x1` holds the previous OUTPUT (the restored x), `y1` the previous input.
    fn de(&mut self, y: f32, c: (f32, f32, f32)) -> f32 {
        let x = (y - c.1 * self.x1 + c.2 * self.y1) / c.0;
        self.x1 = x;
        self.y1 = y;
        x
    }
}

/// One channel's state.
#[derive(Clone)]
struct Chan {
    /// The recorded (encoded) signal at the tape's input, a ring for the flutter's fractional read.
    lin_ring: Vec<f32>,
    hifi_ring: Vec<f32>,
    /// The linear track's played signal (before the loss filter).
    lin_fir: [f32; 2 * LIN_M + 1],
    dolby_enc: DolbyB,
    dolby_dec: DolbyB,
    comp_env: f32,
    exp_env: f32,
    pre: Shelf,
    de: Shelf,
    /// The FM modulator's phase (cycles) and the receiver's previous sample.
    phase: f64,
    prev: (f32, f32),
    held: f32,
}

impl Chan {
    fn new() -> Chan {
        Chan {
            lin_ring: vec![0.0; RING],
            hifi_ring: vec![0.0; RING],
            lin_fir: [0.0; 2 * LIN_M + 1],
            dolby_enc: DolbyB::default(),
            dolby_dec: DolbyB::default(),
            comp_env: 0.0,
            exp_env: 0.0,
            pre: Shelf::default(),
            de: Shelf::default(),
            phase: 0.0,
            prev: (1.0, 0.0),
            held: 0.0,
        }
    }
}

const RING: usize = 256;

/// Dolby B as a sliding high band: encode `y = x + g(e)·H(x)`, decode solves `x = y − g(e)·H(x)`
/// for the current sample (H is first order). The level detector follows the band's own output.
#[derive(Clone, Copy, Default)]
struct DolbyB {
    /// The high-pass state: the previous input and output.
    hx: f32,
    hy: f32,
    env: f32,
}

impl DolbyB {
    /// The high pass's coefficients: H(x)[n] = a·(hy + x − hx).
    fn a() -> f32 {
        let rc = 1.0 / (std::f64::consts::TAU * DOLBY_HZ);
        (rc / (rc + 1.0 / FS)) as f32
    }
    fn gain(&self) -> f32 {
        DOLBY_G / (1.0 + self.env / DOLBY_E0)
    }
    fn track(&mut self, band: f32) {
        let r = band.abs();
        let k = if r > self.env { 0.05 } else { 0.0004 };
        self.env += k * (r - self.env);
    }
    fn encode(&mut self, x: f32, a: f32) -> f32 {
        let band = a * (self.hy + x - self.hx);
        self.hx = x;
        self.hy = band;
        let y = x + self.gain() * band;
        self.track(band);
        y
    }
    fn decode(&mut self, y: f32, a: f32) -> f32 {
        // band = a·x + s, s = a·(hy − hx): x = (y − g·s) / (1 + g·a)
        let s = a * (self.hy - self.hx);
        let g = self.gain();
        let x = (y - g * s) / (1.0 + g * a);
        let band = a * x + s;
        self.hx = x;
        self.hy = band;
        self.track(band);
        x
    }
}

/// What the deck reads on one picture line: the carrier amplitude of each Hi-Fi channel, the
/// playing head, and the dropout (seconds from the line's start, length; none — `len <= 0`).
#[derive(Clone, Copy, Debug)]
pub struct LineRead {
    pub amp: [f32; 2],
    pub head: u32,
    pub drop_start: f32,
    pub drop_len: f32,
}

/// Where a timeline moment falls on the tape: the tape frame, the field, the line of the field, the
/// time since that line began (s), and whether the head has switched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapePos {
    pub frame: i64,
    pub field: u32,
    pub line: u32,
    pub in_line: f64,
}

/// The tape position of timeline time `t` (s).
pub fn tape_pos(d: &Deck, t: f64) -> TapePos {
    let st = d.st;
    let fr = st.frame_rate;
    let x = t * fr;
    let frame = x.floor();
    let in_frame = (x - frame) / fr;
    let half = 0.5 / fr;
    let field = (in_frame >= half) as u32;
    let tau = in_frame - field as f64 * half;
    let line = (tau / st.t_line).floor();
    TapePos { frame: frame as i64, field, line: line as u32, in_line: tau - line * st.t_line }
}

/// The picture's parameter words of tape frame `frame` (`Deck::words` at the frame's middle).
pub fn frame_words(d: &Deck, frame: i64) -> [u32; WORDS] {
    let tick = ((frame as f64 + 0.5) / d.st.frame_rate * 90_000.0).round() as i64;
    d.words(tick)
}

/// The head (spacing m, gap m, output linear) of drum `dr`, head `h`; with one deck the record drum
/// plays too.
fn head_um(d: &Deck, dr: usize, h: usize) -> (f64, f64, f64) {
    let r = row::head(if d.two_decks() { dr } else { 0 }, h);
    let v = &d.v;
    (v[r].max(0.0) * 1e-6, v[r + 1].max(0.01) * 1e-6, 10f64.fpowf(v[r + 2].min(12.0) / 20.0))
}

/// The read of picture line `y` (0..h) of the frame with words `c`.
pub fn line_read(d: &Deck, c: &[u32; WORDS], y: u32) -> LineRead {
    let g = deck::row_geo(c, y);
    let car = hifi_carriers(d.st.std);
    let (rs, _, ro) = head_um(d, 0, g.rec as usize);
    let (ps, pg, po) = head_um(d, 1, g.play as usize);
    let cov = (g.c0 / C0_REF).min(1.0 / C0_REF);
    let mut amp = [0f32; 2];
    for (a, f) in amp.iter_mut().zip(car) {
        let lam = d.st.v_write / f;
        *a = (ro * po * spacing_loss(rs + ps, lam) * gap_loss(pg, lam).abs()) as f32 * cov;
    }
    let start = deck::lt_scalar(c, y, deck::lt::DROP_START);
    let (drop_start, drop_len) = if start >= 0.0 {
        let len = deck::lt_scalar(c, y, deck::lt::DROP_LEN);
        ((start as f64 / d.st.fs) as f32, (len as f64 / d.st.fs) as f32)
    } else {
        (0.0, -1.0)
    };
    LineRead { amp, head: g.play, drop_start, drop_len }
}

/// The audio state of `vhs_tape_sound`.
#[derive(Clone)]
pub struct TapeSound {
    v: Vec<f64>,
    deck: Deck,
    at: Placement,
    ch: [Chan; 2],
    lin_key: Option<[u64; 4]>,
    fir: [f32; 2 * LIN_M + 1],
    /// The ring's write index (frames since the last reset).
    w: usize,
    /// The cached words and line reads: (frame, words), ((frame, y), read).
    words: Option<(i64, Box<[u32; WORDS]>)>,
    line: Option<((i64, u32), LineRead)>,
    shelf: (f32, f32, f32),
    dolby_a: f32,
    /// The compander's level detectors: a fast attack, a slower release (as a noise reduction's
    /// detector is built: a loud onset is caught at once, a gap does not pump the next words).
    att_k: f32,
    rel_k: f32,
}

impl TapeSound {
    pub fn new() -> TapeSound {
        let v = crate::defaults();
        TapeSound {
            deck: Deck::new(&v),
            v,
            at: Placement::default(),
            ch: [Chan::new(), Chan::new()],
            lin_key: None,
            fir: [0.0; 2 * LIN_M + 1],
            w: 0,
            words: None,
            line: None,
            shelf: shelf_coefs(),
            dolby_a: DolbyB::a(),
            att_k: one_pole(160.0),
            rel_k: one_pole(8.0),
        }
    }

    fn pick(&self, r: usize) -> u32 {
        self.v[r].round().max(0.0) as u32
    }

    fn refresh_linear(&mut self) {
        let speed = self.pick(row::SPEED);
        let gap = self.v[row::AC_GAP].max(0.05) * 1e-6;
        // over-bias erases the short wavelengths as it records them: an extra spacing
        let spacing = self.v[row::AC_SPACING].max(0.0) * 1e-6 + 0.4e-6 * (self.v[row::BIAS] - 1.0).max(0.0);
        let key = [speed as u64, self.deck.st.std as u64, gap.to_bits(), spacing.to_bits()];
        if self.lin_key == Some(key) {
            return;
        }
        self.lin_key = Some(key);
        let v_sp = tape_speed(self.deck.st.std, 0);
        self.fir = design_linear(tape_speed(self.deck.st.std, speed), v_sp, gap, spacing);
    }

    fn words_of(&mut self, frame: i64) -> [u32; WORDS] {
        if let Some((f, w)) = &self.words
            && *f == frame
        {
            return **w;
        }
        let w = frame_words(&self.deck, frame);
        self.words = Some((frame, Box::new(w)));
        self.line = None;
        w
    }

    fn read_at(&mut self, p: TapePos) -> LineRead {
        let hf = self.deck.st.h / 2;
        let y = 2 * p.line.min(hf - 1) + p.field;
        if let Some((k, r)) = self.line
            && k == (p.frame, y)
        {
            return r;
        }
        let c = self.words_of(p.frame);
        let r = line_read(&self.deck, &c, y);
        self.line = Some(((p.frame, y), r));
        r
    }

    /// The Hi-Fi time-base error (samples) at time `t`: the picture's drum and capstan components
    /// (`deck::lt_scalar`'s TBE without the per-line grain, which is 10⁻³ of a sample here).
    fn hifi_displacement(&self, t: f64) -> f64 {
        let j = self.v[row::JITTER].max(0.0);
        let drum = (self.deck.st.drum_rate * t).fract();
        let flut = (FLUTTER_HZ * t + FLUTTER_PH).fract();
        j * 1e-6 * FS * (0.6 * sin_cyc(drum as f32) as f64 + 0.5 * sin_cyc(flut as f32) as f64)
    }

    /// The linear track's displacement (samples): the capstan's flutter and the head's scrape.
    fn linear_displacement(&self, t: f64) -> f64 {
        let j = self.v[row::JITTER].max(0.0);
        let flut = (FLUTTER_HZ * t + FLUTTER_PH).fract();
        let cell = (t / SCRAPE_CELL).floor();
        let fr = t / SCRAPE_CELL - cell;
        let r0 = gauss(hash3(cell as i64 as u32, 0x5C4A, 1)) as f64;
        let r1 = gauss(hash3((cell as i64 + 1) as u32, 0x5C4A, 1)) as f64;
        j * (CAP_SAMPLES_PER_US * sin_cyc(flut as f32) as f64 + SCRAPE_SAMPLES_PER_US * (r0 + (r1 - r0) * fr))
    }

    /// The linear track's dropout depth (0 — none) at time `t` on track `k`: oxide defects at the
    /// picture's rate scaled by the areas the two tracks sweep, each `L = dropout_us · v_write` long
    /// on the tape, passing the stationary head in `L / v_tape`, as deep as they cover the track.
    pub fn linear_dropout(&self, t: f64, k: u32) -> f32 {
        let st = self.deck.st;
        let stereo = self.pick(row::LINEAR_TRACKS) == 1;
        let w = linear_width(stereo);
        let v = tape_speed(st.std, self.pick(row::SPEED));
        let rate = self.v[row::DROPOUTS].max(0.0) * (w * v) / (st.pitch_sp * st.v_write);
        let len_m = self.v[row::DROPOUT_LEN].max(0.0) * 1e-6 * st.v_write;
        let dur = len_m / v;
        let depth = (len_m / w).min(1.0) as f32;
        const CELL: f64 = 0.001;
        let p = (rate * CELL).min(1.0) as f32;
        let c0 = (t / CELL).floor() as i64;
        let back = (dur / CELL).ceil() as i64 + 1;
        for c in (c0 - back)..=c0 {
            let h = hash3(c as u32, (c >> 32) as u32 ^ 0x11D0, 7 + k);
            if unit(h) < p {
                let s = (c as f64 + unit(hash(h ^ 3)) as f64) * CELL;
                if t >= s && t < s + dur {
                    return depth;
                }
            }
        }
        0.0
    }

    fn reset_chans(&mut self) {
        self.ch = [Chan::new(), Chan::new()];
        self.w = 0;
        self.words = None;
        self.line = None;
    }
}

impl Default for TapeSound {
    fn default() -> TapeSound {
        TapeSound::new()
    }
}

/// A fractional read of the ring `d` samples behind the newest (linear interpolation).
#[inline]
fn ring_read(ring: &[f32], w: usize, d: f64) -> f32 {
    let i = d.floor();
    let fr = (d - i) as f32;
    let a = ring[(w + RING - i as usize % RING) % RING];
    let b = ring[(w + RING - (i as usize + 1) % RING) % RING];
    a + (b - a) * fr
}

impl AudioState for TapeSound {
    fn apply(&mut self, k: &[f64], structural: bool) {
        let mut v = crate::defaults();
        for (o, x) in v.iter_mut().zip(k) {
            *o = *x;
        }
        if v != self.v || structural {
            self.deck = Deck::new(&v);
            self.v = v;
            self.words = None;
            self.line = None;
        }
        self.refresh_linear();
        if structural {
            self.reset_chans();
        }
    }

    fn set_targets(&mut self, c: &dyn kt_plugin_api::SlotCurves, s: f64) {
        // the deck's numbers follow their curves chunk by chunk (tracking turned during a take)
        let specs = crate::passport();
        let mut changed = false;
        let mut v = self.v.clone();
        for (i, spec) in specs.iter().enumerate() {
            if spec.kind == kt_plugin_api::ParamKind::Smooth {
                let x = c.value(&spec.name, v[i], s);
                if x != v[i] {
                    v[i] = x;
                    changed = true;
                }
            }
        }
        if changed {
            self.deck = Deck::new(&v);
            self.v = v;
            self.words = None;
            self.line = None;
            self.refresh_linear();
        }
    }

    fn place(&mut self, at: Placement) {
        self.at = at;
    }

    fn lookahead(&self) -> i64 {
        LA
    }

    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }

    fn reset(&mut self) {
        self.reset_chans();
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let out_sel = self.pick(row::AUDIO_OUT).min(2);
        let still = self.deck.still();
        let stereo_lin = self.pick(row::LINEAR_TRACKS) == 1;
        let dolby = self.v[row::DOLBY] >= 0.5;
        let doc = self.deck.doc();
        let gain = 10f32.fpowf(self.v[row::REC_LEVEL] as f32 / 20.0);
        // the bias: under-bias bends the transfer curve (odd harmonics); the oxide's own saturation
        // (the picture's FM saturates on the same coating) bends it too
        let drive = (0.8 + 3.0 * (1.0 - self.v[row::BIAS] as f32).max(0.0))
            * (0.6 + 1.6 * self.v[row::SATURATION] as f32);
        let st = self.deck.st;
        let width = linear_width(stereo_lin);
        let snr = LIN_SNR_DB + (self.v[row::CN] - CN_REF_DB) + 10.0 * (width / 1.0e-3).flog10();
        let hiss = REF * 10f32.fpowf(-(snr as f32) / 20.0);
        let cn = 10f32.fpowf((self.v[row::CN] - HIFI_CN_BELOW) as f32 / 10.0);
        let sigma = (0.5 / cn).sqrt();
        let car = hifi_carriers(st.std);
        let skew = self.v[row::SWITCH_SKEW] * 1e-6;
        let sp = self.v[row::SWITCH_POINT];
        let hf = (st.h / 2) as f64;
        let dev_n = DEV / (FS * OS as f64);
        let shelf = self.shelf;
        let da = self.dolby_a;
        let (att_k, rel_k) = (self.att_k, self.rel_k);
        let follow = |env: &mut f32, p: f32| *env += if p > *env { att_k } else { rel_k } * (p - *env);
        let fir = self.fir;
        let frames = buf.len() / 2;
        for i in 0..frames {
            // the INPUT position runs LA ahead; the device is read at the OUTPUT's time
            let n_out = self.at.start + pos + i as i64 - LA;
            let t = n_out as f64 / FS;
            let x = [buf[2 * i], buf[2 * i + 1]];
            // --- record: both systems write the same input ---------------------------------------
            let lin_in = if stereo_lin { x } else { [0.5 * (x[0] + x[1]); 2] };
            self.w = (self.w + 1) % RING;
            let w = self.w;
            for (k, c) in self.ch.iter_mut().enumerate() {
                let mut s = lin_in[k] * gain;
                if dolby {
                    s = c.dolby_enc.encode(s, da);
                }
                c.lin_ring[w] = tanh(drive * s / REF) * REF / drive;
                // Hi-Fi encoder: emphasis, then the 2:1 compressor on the RMS
                let e = c.pre.pre(x[k], shelf);
                follow(&mut c.comp_env, e * e);
                let g = (c.comp_env.sqrt().max(1e-5) / REF).fpowf(-0.5);
                c.hifi_ring[w] = (e * g).clamp(-1.0, 1.0);
            }
            if still {
                // a stopped tape under a still head plays nothing; the deck mutes Hi-Fi in trick play
                buf[2 * i] = 0.0;
                buf[2 * i + 1] = 0.0;
                continue;
            }
            // --- the linear track -----------------------------------------------------------------
            let mut lin = [0f32; 2];
            if out_sel != 0 {
                // the tape passes the head LIN_M samples before the loss filter's output
                let d_lin = (LA - LIN_M as i64) as f64 + self.linear_displacement(t).clamp(-56.0, 56.0);
                let t_head = t - LIN_M as f64 / FS;
                let depth = [self.linear_dropout(t_head, 0), self.linear_dropout(t_head, stereo_lin as u32)];
                for (k, c) in self.ch.iter_mut().enumerate() {
                    let played = ring_read(&c.lin_ring, w, d_lin) * (1.0 - depth[k]);
                    let hn = hiss * gauss(hash3(n_out as u32, (n_out >> 32) as u32, 0xA1 + if stereo_lin { k as u32 } else { 0 }));
                    c.lin_fir.copy_within(1.., 0);
                    c.lin_fir[2 * LIN_M] = played + hn;
                    let mut y = 0f32;
                    for (a, b) in fir.iter().zip(c.lin_fir.iter()) {
                        y += a * b;
                    }
                    if dolby {
                        y = c.dolby_dec.decode(y, da);
                    }
                    lin[k] = y / gain;
                }
            }
            // --- Hi-Fi ------------------------------------------------------------------------------
            let mut hifi = [0f32; 2];
            if out_sel != 1 {
                let d_hifi = LA as f64 + self.hifi_displacement(t);
                let p = tape_pos(&self.deck, t);
                let rd = self.read_at(p);
                // the switch: from line `hf − sp` of the field's active part the next head reads
                // (`deck::row_geo`); the deck's switching pulse holds the sample it falls in
                let t_sw = (hf - sp) * st.t_line;
                let tau = p.line as f64 * st.t_line + p.in_line;
                let at_switch = (tau - t_sw).abs() < 0.5 / FS;
                for (k, c) in self.ch.iter_mut().enumerate() {
                    let m = ring_read(&c.hifi_ring, w, d_hifi);
                    let a = rd.amp[k];
                    // the receiver's phase offset: the playing head's track is `skew` late
                    let off = car[k] * skew * rd.head as f64;
                    let mut acc = 0f32;
                    let mut env = 0f32;
                    for s in 0..OS {
                        c.phase += dev_n * m as f64;
                        c.phase -= c.phase.floor();
                        let sub_t = p.in_line + s as f64 / (FS * OS as f64);
                        let dropped = rd.drop_len > 0.0
                            && sub_t >= rd.drop_start as f64
                            && sub_t < (rd.drop_start + rd.drop_len) as f64;
                        let aa = if dropped { 0.0 } else { a };
                        let ph = (c.phase + off).fract() as f32;
                        let ni = (n_out as u32).wrapping_mul(OS as u32).wrapping_add(s as u32);
                        let hn = hash3(ni, (n_out >> 29) as u32, 0xF0 + k as u32);
                        let z = (aa * cos_cyc(ph) + sigma * gauss(hn), aa * sin_cyc(ph) + sigma * gauss(hn ^ 0x9E37));
                        let (pr, pi) = c.prev;
                        let re = z.0 * pr + z.1 * pi;
                        let im = z.1 * pr - z.0 * pi;
                        acc += atan2_cyc(im, re);
                        env += z.0 * z.0 + z.1 * z.1;
                        c.prev = z;
                    }
                    let demod = acc / (OS as f32 * dev_n as f32);
                    let lost = env / OS as f32 / (2.0 * sigma * sigma) < CARRIER_DETECT;
                    // the switching pulse holds the sample of the head switch; with the DOC on the
                    // carrier detector mutes the Hi-Fi output while there is no carrier
                    let d = if at_switch {
                        c.held
                    } else if doc && lost {
                        0.0
                    } else {
                        demod.clamp(-1.5, 1.5)
                    };
                    c.held = d;
                    follow(&mut c.exp_env, d * d);
                    let e = d * (c.exp_env.sqrt().max(1e-5 / REF) / REF);
                    hifi[k] = c.de.de(e, shelf);
                }
            }
            let out = match out_sel {
                0 => hifi,
                1 => lin,
                _ => [0.5 * (hifi[0] + lin[0]), 0.5 * (hifi[1] + lin[1])],
            };
            buf[2 * i] = out[0];
            buf[2 * i + 1] = out[1];
        }
    }
}
