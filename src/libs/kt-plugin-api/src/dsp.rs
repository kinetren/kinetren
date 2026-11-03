//! Reusable DSP primitives of the audio nodes (R.47, Ka1; moved here from kt-fx by
//! plugin-api-stage1, D2a, so the built-in and the core nodes share one text): smoothing, delay lines,
//! filters, envelopes. Everything is deterministic and lock-free —
//! there is one owner of the state (the audio-render thread, invariant 2).

/// The engine's time unit, for a family's `tail` (a family has no kt-time of its own).
use crate::math::FrozenMath;
pub use kt_time::Tick;

/// Smoothed parameter: one-pole toward the target, ~20 ms. It does not “teleport”
/// on set_target — only in reset().
#[derive(Debug, Clone, Copy)]
pub struct SmoothedParam {
    current: f32,
    pub target: f32,
    alpha: f32,
}

impl SmoothedParam {
    pub fn new(value: f32) -> SmoothedParam {
        // 1 - exp(-1 / (0.020 * 48000))
        SmoothedParam { current: value, target: value, alpha: 1.0 - (-1.0f32 / 960.0).fexp() }
    }
    #[inline]
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
    #[inline]
    pub fn value(&self) -> f32 {
        self.current
    }
}

// ---------------------------------------------------------------------------
// Freeverb
// ---------------------------------------------------------------------------

/// Classic freeverb tunings (44.1 kHz), scaled to 48 kHz
/// and the delay parameter (base point delay_ms = 100).
pub const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
pub const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
pub const STEREO_SPREAD: usize = 23;

#[derive(Clone)]
pub struct Comb {
    pub buf: Vec<f32>,
    pos: usize,
    pub filter_state: f32,
}

impl Comb {
    pub fn new(len: usize) -> Comb {
        Comb { buf: vec![0.0; len.max(1)], pos: 0, filter_state: 0.0 }
    }
    #[inline]
    pub fn process(&mut self, input: f32, damp: f32, feedback: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_state = out * (1.0 - damp) + self.filter_state * damp;
        self.buf[self.pos] = input + self.filter_state * feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

#[derive(Clone)]
pub struct Allpass {
    pub buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    pub fn new(len: usize) -> Allpass {
        Allpass { buf: vec![0.0; len.max(1)], pos: 0 }
    }
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
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

pub fn tail_frames_for(max_comb_len: f32, feedback: f32) -> Tick {
    let fb = feedback.clamp(0.01, 0.98);
    let db_per_pass = -20.0 * fb.flog10();
    Tick::from_audio_frames((max_comb_len.max(8.0) * (60.0 / db_per_pass)) as i64)
}

// ---------------------------------------------------------------------------
// R.47 primitives (item 324): biquad, delay line, LFO, envelope, softclip
// ---------------------------------------------------------------------------

/// Interleaved stereo: the engine's audio is f32 48 kHz/2ch (invariant 1).
pub const CH: usize = 2;
pub use kt_time::AUDIO_SAMPLE_RATE;

/// The sub-block of filter re-tuning from smoothed parameters, frames (R.47: a chunk of 1024 —
/// 21 ms — is audible as zipper on sweeps).
pub const TUNE_STEP: usize = 128;

impl SmoothedParam {
    /// Smoothing with a CONFIGURABLE time (R.47): the attack/release of the
    /// dynamics and the tape glide of the delay time do not live on a fixed 20 ms.
    /// `new(v)` == `with_ms(v, 20.0)` verbatim.
    pub fn with_ms(value: f32, ms: f32) -> SmoothedParam {
        let frames = (ms.max(0.01) / 1000.0) * AUDIO_SAMPLE_RATE as f32;
        SmoothedParam { current: value, target: value, alpha: 1.0 - (-1.0f32 / frames).fexp() }
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
        let (sin, cos) = w0.fsin_cos();
        let alpha = sin / (2.0 * q);
        let a = 10f32.fpowf(gain_db / 40.0);
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
        let (c1, c2) = (w.fcos(), (2.0 * w).fcos());
        let (b0, b1, b2) = (self.b0 as f64, self.b1 as f64, self.b2 as f64);
        let (a1, a2) = (self.a1 as f64, self.a2 as f64);
        let num = b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * c1 + 2.0 * b0 * b2 * c2;
        let den = 1.0 + a1 * a1 + a2 * a2 + 2.0 * (a1 + a1 * a2) * c1 + 2.0 * a2 * c2;
        10.0 * (num.max(1e-30) / den.max(1e-30)).flog10()
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
#[inline]
pub fn lfo_phase(
    base_hz: f64,
    points: &crate::curves::ParamPoints,
    tau: f64,
    clip_len_frames: i64,
) -> f64 {
    crate::curves::param_integral(base_hz, points, tau) * clip_len_frames as f64
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
            1.0 - (-1.0f32 / frames).fexp()
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

// ---------------------------------------------------------------------------
// The stereo freeverb (R.47's node, moved verbatim from kt-fx in plugin-api-stage1, 3b.2)
// ---------------------------------------------------------------------------

/// Stereo freeverb. Changing delay re-creates the lines (this is a structural change of
/// the geometry — unavoidable), but wet/dry are smoothed and do NOT touch the buffers.
#[derive(Clone)]
pub struct Freeverb {
    combs: [Vec<Comb>; CH],
    allpasses: [Vec<Allpass>; CH],
    wet: SmoothedParam,
    delay_ms: f32,
    damp: f32,
    feedback: f32,
    wet_gain: f32,
    /// Pre-delay of the reverb input (R.33 C5): a ring per channel. Empty —
    /// the previous path byte-for-byte.
    predelay: [Vec<f32>; CH],
    pre_pos: usize,
    predelay_ms: f32,
    /// Stereo tail width 0..1: 1 — the previous “real” stereo reverb,
    /// 0 — a common mono tail for both channels.
    width: f32,
}

impl Freeverb {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mix: f32,
        delay_ms: f32,
        damp: f32,
        decay: f32,
        wet_gain: f32,
        predelay_ms: f32,
        width: f32,
    ) -> Freeverb {
        let scale = |base: usize, extra: usize| -> usize {
            let s = base as f32 * (AUDIO_SAMPLE_RATE as f32 / 44_100.0) * (delay_ms / 100.0);
            (s as usize + extra).max(8)
        };
        let combs = std::array::from_fn(|ch| {
            COMB_TUNINGS
                .iter()
                .map(|&t| Comb::new(scale(t, ch * STEREO_SPREAD)))
                .collect::<Vec<_>>()
        });
        let allpasses = std::array::from_fn(|ch| {
            ALLPASS_TUNINGS
                .iter()
                .map(|&t| Allpass::new(scale(t, ch * STEREO_SPREAD)))
                .collect::<Vec<_>>()
        });
        let pre = (predelay_ms.clamp(0.0, 200.0) / 1000.0 * AUDIO_SAMPLE_RATE as f32) as usize;
        Freeverb {
            combs,
            allpasses,
            wet: SmoothedParam::new(mix.clamp(0.0, 1.0)),
            delay_ms,
            damp: damp.clamp(0.0, 0.99),
            feedback: decay.clamp(0.0, 0.98),
            wet_gain: wet_gain.max(0.0),
            predelay: std::array::from_fn(|_| vec![0.0; pre]),
            pre_pos: 0,
            predelay_ms: predelay_ms.clamp(0.0, 200.0),
            width: width.clamp(0.0, 1.0),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_params(
        &mut self,
        mix: f32,
        delay_ms: f32,
        damp: f32,
        decay: f32,
        wet_gain: f32,
        predelay_ms: f32,
        width: f32,
    ) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
        self.damp = damp.clamp(0.0, 0.99);
        self.feedback = decay.clamp(0.0, 0.98);
        self.wet_gain = wet_gain.max(0.0);
        self.width = width.clamp(0.0, 1.0);
        // structural change of geometry: re-creation is allowed only for a
        // really different delay (threshold 0.5 ms, as in the old code, but wet/dry
        // does not touch this); pre-delay — the same geometry
        if (delay_ms - self.delay_ms).abs() > 0.5 || (predelay_ms - self.predelay_ms).abs() > 0.5 {
            let keep_wet = self.wet;
            *self =
                Freeverb::new(keep_wet.target, delay_ms, damp, decay, wet_gain, predelay_ms, width);
            self.wet = keep_wet;
        }
    }

    /// Only the wet-mix target (automation) — smoothed, buffers not touched.
    pub fn set_mix(&mut self, mix: f32) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
    }

    /// Tail: decay time to −60 dB (t60) along the longest comb line.
    pub fn tail(&self) -> Tick {
        let max_len = self.combs[CH - 1].iter().map(|c| c.buf.len()).max().unwrap_or(1);
        tail_frames_for(max_len as f32, self.feedback)
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        // the previous path (without pre-delay, full width) remains
        // VERBATIM: GOLDEN_AUDIO relies on it
        let plain = self.predelay[0].is_empty() && self.width >= 1.0;
        for frame in samples.chunks_exact_mut(CH) {
            let wet_gain = self.wet.tick();
            if plain {
                for (ch, s) in frame.iter_mut().enumerate() {
                    let dry = *s;
                    let input = dry * 0.015;
                    let mut wet = 0.0;
                    for comb in &mut self.combs[ch] {
                        wet += comb.process(input, self.damp, self.feedback);
                    }
                    for ap in &mut self.allpasses[ch] {
                        wet = ap.process(wet);
                    }
                    *s = dry * (1.0 - wet_gain) + wet * self.wet_gain * wet_gain;
                }
                continue;
            }
            let mut wets = [0.0f32; CH];
            for ch in 0..CH {
                let dry = frame[ch];
                let mut input = dry * 0.015;
                if !self.predelay[ch].is_empty() {
                    let pos = self.pre_pos % self.predelay[ch].len();
                    input = std::mem::replace(&mut self.predelay[ch][pos], input);
                }
                let mut wet = 0.0;
                for comb in &mut self.combs[ch] {
                    wet += comb.process(input, self.damp, self.feedback);
                }
                for ap in &mut self.allpasses[ch] {
                    wet = ap.process(wet);
                }
                wets[ch] = wet;
            }
            self.pre_pos = self.pre_pos.wrapping_add(1);
            // width: 1 — channels as they are, 0 — a common mono tail
            let (a, b) = (0.5 + 0.5 * self.width, 0.5 - 0.5 * self.width);
            for ch in 0..CH {
                let other = wets[(ch + 1) % CH];
                let wet = wets[ch] * a + other * b;
                let dry = frame[ch];
                frame[ch] = dry * (1.0 - wet_gain) + wet * self.wet_gain * wet_gain;
            }
        }
    }

    pub fn reset(&mut self) {
        for ch in 0..CH {
            for c in &mut self.combs[ch] {
                c.buf.fill(0.0);
                c.filter_state = 0.0;
            }
            for a in &mut self.allpasses[ch] {
                a.buf.fill(0.0);
            }
            // predelay also holds material (up to 200 ms) — without clearing it
            // it leaked into the tail after a seek (t319, R.47 item 324)
            self.predelay[ch].fill(0.0);
        }
        self.pre_pos = 0;
        self.wet.reset_to_target();
    }
}

/// The tail of a freeverb with this delay and feedback, before the lines exist (the static
/// `chain_tail`/`Decl.tail` formula; it agrees with `Freeverb::tail`, t322f).
pub fn reverb_tail_for_delay(delay_ms: f32, decay: f32) -> Tick {
    let max_comb = COMB_TUNINGS[COMB_TUNINGS.len() - 1] as f32
        * (AUDIO_SAMPLE_RATE as f32 / 44_100.0)
        * (delay_ms / 100.0)
        + ((CH - 1) * STEREO_SPREAD) as f32;
    tail_frames_for(max_comb, decay)
}
