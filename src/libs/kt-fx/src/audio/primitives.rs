//! Reusable DSP primitives of the audio nodes (R.47, Ka1): smoothing, delay lines,
//! filters, envelopes. Everything is deterministic and lock-free —
//! there is one owner of the state (the audio-render thread, invariant 2).

use kt_time::Tick;

/// Smoothed parameter: one-pole toward the target, ~20 ms. It does not “teleport”
/// on set_target — only in reset().
#[derive(Debug, Clone, Copy)]
pub struct SmoothedParam {
    current: f32,
    pub(crate) target: f32,
    alpha: f32,
}

impl SmoothedParam {
    pub fn new(value: f32) -> SmoothedParam {
        // 1 - exp(-1 / (0.020 * 48000))
        SmoothedParam { current: value, target: value, alpha: 1.0 - (-1.0f32 / 960.0).exp() }
    }
    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }
    pub fn reset_to_target(&mut self) {
        self.current = self.target;
    }
    #[inline]
    pub fn tick(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.alpha;
        self.current
    }
    pub fn value(&self) -> f32 {
        self.current
    }
}

// ---------------------------------------------------------------------------
// Freeverb
// ---------------------------------------------------------------------------

/// Classic freeverb tunings (44.1 kHz), scaled to 48 kHz
/// and the delay parameter (base point delay_ms = 100).
pub(crate) const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
pub(crate) const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
pub(crate) const STEREO_SPREAD: usize = 23;

pub(crate) struct Comb {
    pub(crate) buf: Vec<f32>,
    pos: usize,
    pub(crate) filter_state: f32,
}

impl Comb {
    pub(crate) fn new(len: usize) -> Comb {
        Comb { buf: vec![0.0; len.max(1)], pos: 0, filter_state: 0.0 }
    }
    #[inline]
    pub(crate) fn process(&mut self, input: f32, damp: f32, feedback: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_state = out * (1.0 - damp) + self.filter_state * damp;
        self.buf[self.pos] = input + self.filter_state * feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

pub(crate) struct Allpass {
    pub(crate) buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    pub(crate) fn new(len: usize) -> Allpass {
        Allpass { buf: vec![0.0; len.max(1)], pos: 0 }
    }
    #[inline]
    pub(crate) fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.pos];
        let out = bufout - input;
        self.buf[self.pos] = input + bufout * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

// ---------------------------------------------------------------------------
// Clip processor: an FX chain over the (possibly warped) source
// ---------------------------------------------------------------------------

pub(crate) fn tail_frames_for(max_comb_len: f32, feedback: f32) -> Tick {
    let fb = feedback.clamp(0.01, 0.98);
    let db_per_pass = -20.0 * fb.log10();
    Tick::from_audio_frames((max_comb_len.max(8.0) * (60.0 / db_per_pass)) as i64)
}

// ---------------------------------------------------------------------------
// R.47 primitives (item 324): biquad, delay line, LFO, envelope, softclip
// ---------------------------------------------------------------------------

use crate::CH;
use kt_time::AUDIO_SAMPLE_RATE;

impl SmoothedParam {
    /// Smoothing with a CONFIGURABLE time (R.47): the attack/release of the
    /// dynamics and the tape glide of the delay time do not live on a fixed 20 ms.
    /// `new(v)` == `with_ms(v, 20.0)` verbatim.
    pub fn with_ms(value: f32, ms: f32) -> SmoothedParam {
        let frames = (ms.max(0.01) / 1000.0) * AUDIO_SAMPLE_RATE as f32;
        SmoothedParam { current: value, target: value, alpha: 1.0 - (-1.0f32 / frames).exp() }
    }
}

/// RBJ biquad (transposed direct form II), one channel. Coefficients
/// are recalculated in sub-blocks (128 frames) from the smoothed parameters —
/// a chunk of 1024 (21 ms) is audible as zipper on sweeps.
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

pub enum BiquadKind {
    Lowpass,
    Highpass,
    Bandpass,
    LowShelf,
    Peak,
    HighShelf,
}

impl Biquad {
    /// Recalculate the coefficients WITHOUT touching the state (z1/z2 live on —
    /// a frequency sweep does not click).
    pub fn tune(&mut self, kind: &BiquadKind, freq_hz: f32, q: f32, gain_db: f32) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let f = freq_hz.clamp(10.0, sr * 0.49);
        let q = q.max(0.05);
        let w0 = 2.0 * std::f32::consts::PI * f / sr;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a = 10f32.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::Lowpass => {
                let c = (1.0 - cos) * 0.5;
                (c, 1.0 - cos, c, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::Highpass => {
                let c = (1.0 + cos) * 0.5;
                (c, -(1.0 + cos), c, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Peak => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            BiquadKind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - s),
                    (a + 1.0) + (a - 1.0) * cos + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - s,
                )
            }
            BiquadKind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - s),
                    (a + 1.0) - (a - 1.0) * cos + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - s,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// The frequency response in dB at `freq_hz` — from the CURRENT coefficients, that is
    /// from the very filter that is sounding now (including the
    /// f32 quantization of `tune`). The RBJ formulas are NOT duplicated here: the graph
    /// in the UI by construction cannot diverge from the DSP, and an edit to `tune` itself
    /// is reflected in the picture.
    ///
    /// |H(e^jw)|² = |B|²/|A|² expanded via cos(w) and cos(2w) — without a complex type
    /// and without dependencies.
    pub fn mag_db(&self, freq_hz: f32) -> f64 {
        let w = 2.0 * std::f64::consts::PI * freq_hz.max(0.0) as f64 / AUDIO_SAMPLE_RATE as f64;
        let (c1, c2) = (w.cos(), (2.0 * w).cos());
        let (b0, b1, b2) = (self.b0 as f64, self.b1 as f64, self.b2 as f64);
        let (a1, a2) = (self.a1 as f64, self.a2 as f64);
        let num = b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * c1 + 2.0 * b0 * b2 * c2;
        let den = 1.0 + a1 * a1 + a2 * a2 + 2.0 * (a1 + a1 * a2) * c1 + 2.0 * a2 * c2;
        10.0 * (num.max(1e-30) / den.max(1e-30)).log10()
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// Circular delay line: it does NOT know stereo-interleaved — it works per channel.
/// Allocated to the MAXIMUM at creation: a time change is a fractional read,
/// not a re-creation (invariant 3). Denormals are flushed on write.
#[derive(Debug, Clone)]
pub struct DelayLine {
    buf: Vec<f32>,
    pos: usize,
}

impl DelayLine {
    pub fn new(max_frames: usize) -> DelayLine {
        DelayLine { buf: vec![0.0; max_frames.max(2)], pos: 0 }
    }

    #[inline]
    pub fn write(&mut self, v: f32) {
        // Flush of denormals: feedback tails on x86 without FTZ cost
        // a clock tick on every subnormal value
        self.buf[self.pos] = if v.abs() < 1e-20 { 0.0 } else { v };
        self.pos = (self.pos + 1) % self.buf.len();
    }

    /// Read the value written `frames` frames BACK (1.0 —
    /// the last write; a fractional delay — linear interpolation).
    /// Node convention: read first (echo from the past), then write.
    #[inline]
    pub fn read_frac(&self, frames: f32) -> f32 {
        let len = self.buf.len();
        let d = frames.clamp(1.0, (len - 1) as f32);
        let i = d.floor() as i64;
        let fr = d - i as f32;
        let i0 = (self.pos as i64 - i).rem_euclid(len as i64) as usize;
        let i1 = (self.pos as i64 - i - 1).rem_euclid(len as i64) as usize;
        self.buf[i0] * (1.0 - fr) + self.buf[i1] * fr
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

/// LFO phase in CYCLES from the clip start: the integral of the frequency curve over the position
/// (R.47, item 324). Not a counter: seek/seam/loop wrap give the same phase as
/// a cold render of the same position (invariant 11) — no LFO warm-up is needed.
pub fn lfo_phase(
    base_hz: f64,
    points: &kt_model::ParamPoints,
    tau: f64,
    clip_len_frames: i64,
) -> f64 {
    kt_model::param_integral(base_hz, points, tau) * clip_len_frames as f64
        / AUDIO_SAMPLE_RATE as f64
}

/// Peak envelope with attack/release (per channel or on the stereo link —
/// decided by the caller).
#[derive(Debug, Clone, Copy)]
pub struct EnvFollower {
    value: f32,
    attack_alpha: f32,
    release_alpha: f32,
}

impl EnvFollower {
    pub fn new(attack_ms: f32, release_ms: f32) -> EnvFollower {
        let a = |ms: f32| {
            let frames = (ms.max(0.01) / 1000.0) * AUDIO_SAMPLE_RATE as f32;
            1.0 - (-1.0f32 / frames).exp()
        };
        EnvFollower { value: 0.0, attack_alpha: a(attack_ms), release_alpha: a(release_ms) }
    }

    pub fn set_times(&mut self, attack_ms: f32, release_ms: f32) {
        let n = EnvFollower::new(attack_ms, release_ms);
        self.attack_alpha = n.attack_alpha;
        self.release_alpha = n.release_alpha;
    }

    #[inline]
    pub fn tick(&mut self, peak: f32) -> f32 {
        let alpha = if peak > self.value { self.attack_alpha } else { self.release_alpha };
        self.value += (peak - self.value) * alpha;
        if self.value.abs() < 1e-20 {
            self.value = 0.0;
        }
        self.value
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
    }
}

/// Soft saturation x/(1+|x|): algebraic (no tanh) — identical on
/// all platforms, determinism for free.
#[inline]
pub fn softclip(v: f32) -> f32 {
    v / (1.0 + v.abs())
}

/// Mid/side width matrix: apply to a stereo frame. width 1.0 is NOT
/// short-circuited here — the node does the short-circuit (bytes).
#[inline]
pub fn apply_width(frame: &mut [f32], width: f32) {
    if CH < 2 {
        return;
    }
    let m = (frame[0] + frame[1]) * 0.5;
    let s = (frame[0] - frame[1]) * 0.5 * width;
    frame[0] = m + s;
    frame[1] = m - s;
}
