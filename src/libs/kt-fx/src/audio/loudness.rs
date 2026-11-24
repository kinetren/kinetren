//! Loudness per ITU-R BS.1770-4 / EBU R 128 (R.49, item 370).
//!
//! What is computed: integrated loudness (LUFS), loudness range
//! (LRA, LU) and true peak (dBTP).
//!
//! **Why everything is computed in f64 — and why NOT for the reason that comes first
//! to mind.** The K-filter coefficients look like
//! `a1 = -1.99004745483398`, and it is tempting to say “in f32 such a tail
//! is cut off, and the biquad is recursive”. MEASURED: truncating the coefficients to f32
//! shifts the result by no more than 0.002 LU at 20…1000 Hz — against the
//! EBU tolerance of 0.1 LU this is not the cause.
//!
//! The real cause is the SUMS. Direct accumulation of squares in f32 over ten
//! minutes of a −20 dBFS tone gives −23.38 instead of −23.01, that is 0.37 LU — three times
//! the tolerance: the accumulator grows, and the next addend simply
//! stops getting into it. The path is offline and not hot, so in f64
//! the whole chain is kept, not each sum separately (guarding
//! “do not convert this sum to f32” would have to be done forever). The ordinary `Biquad`
//! stays in f32 in the effects: there it sounds, rather than measures.
//!
//! **Gates, not averages.** The absolute threshold of −70 LUFS discards silence;
//! the relative −10 LU from the preliminary result discards the pauses
//! between phrases. Without them, dialogue with pauses would measure quieter than music mixed
//! to the same level, and “normalize to −23” would make speech too loud.
//!
//! **True peak, not sample peak.** Between samples the signal can be higher than
//! in them: the reconstructed wave passes ABOVE the points. ×4 oversampling
//! (polyphase FIR) catches this overshoot — otherwise “a −0.1 dBFS peak” turns into
//! clipping after the codec.

use kt_plugin_api::math::FrozenMath;
use kt_time::AUDIO_CHANNELS;

/// Double-precision biquad — for measurements (see the module comment).
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad64 {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad64 {
    pub fn new(b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) -> Biquad64 {
        Biquad64 { b0, b1, b2, a1, a2, z1: 0.0, z2: 0.0 }
    }

    /// Direct form II transposed — the same as `Biquad`.
    pub fn step(&mut self, x: f64) -> f64 {
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

/// K-filter of BS.1770: high-shelf boost + high-pass filter.
/// Coefficients — from the specification for 48 kHz, exactly as printed.
fn k_filter() -> (Biquad64, Biquad64) {
    let shelf = Biquad64::new(
        1.53512485958697,
        -2.69169618940638,
        1.19839281085285,
        -1.69065929318241,
        0.73248077421585,
    );
    let hp = Biquad64::new(1.0, -2.0, 1.0, -1.99004745483398, 0.99007225036621);
    (shelf, hp)
}

/// BS.1770 channel weights. Stereo: both 1.0 (surround and rear weigh more,
/// but we have none — the path is always 2 channels, invariant 1).
const CH_WEIGHT: [f64; 2] = [1.0, 1.0];

/// Measurement block: 400 ms with 75 % overlap (100 ms step).
const BLOCK_MS: usize = 400;
const STEP_MS: usize = 100;
/// Absolute gate.
const ABS_GATE_LUFS: f64 = -70.0;
/// Relative gate from the preliminary loudness.
const REL_GATE_LU: f64 = -10.0;

/// Measurement result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
    /// Integrated loudness, LUFS. `None` — the material is entirely below the
    /// absolute gate (silence): “−70” here would be a lie about a number
    /// that does not exist.
    pub lufs: Option<f64>,
    /// Loudness range, LU (EBU Tech 3342).
    pub lra: f64,
    /// True peak, dBTP.
    pub dbtp: f64,
    /// Each channel on its own (render-loudness-norms, F-EXP-010): BS.1770 SUMS the channels'
    /// energies, so a silent right channel measures only 3 LU quieter than a balanced mix — the
    /// summed value hides it. Measured on the same samples, the audio is not touched.
    pub channels: [ChannelLoudness; AUDIO_CHANNELS],
}

/// One channel of a [`Loudness`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelLoudness {
    /// The channel's level over the blocks the SUMMED measurement's two gates kept (so the two
    /// channels are compared over the same moments), LUFS; `None` — silent over them.
    pub lufs: Option<f64>,
    /// The channel's true peak, dBTP.
    pub dbtp: f64,
}

/// Accumulator: fed with pieces of audio in any slicing order.
///
/// The slicing does NOT AFFECT the result — this is a property, not a coincidence: both
/// the filters and the block grid live in the accumulator, not in the call.
pub struct LoudnessMeter {
    filters: [(Biquad64, Biquad64); AUDIO_CHANNELS],
    /// Sum of squares of the current step (100 ms) across channels.
    step_sum: [f64; AUDIO_CHANNELS],
    step_frames: usize,
    frames_per_step: usize,
    /// Energies of the last steps — a block is 4 consecutive steps.
    steps: std::collections::VecDeque<[f64; AUDIO_CHANNELS]>,
    /// Mean squares of blocks (for gates and LRA).
    blocks: Vec<f64>,
    /// The same blocks' mean squares per channel (unweighted), for `ChannelLoudness`.
    blocks_ch: Vec<[f64; AUDIO_CHANNELS]>,
    /// Short blocks of 3 s for LRA.
    short_steps: std::collections::VecDeque<[f64; AUDIO_CHANNELS]>,
    short_blocks: Vec<f64>,
    /// The true peak per channel (the summed one is their maximum).
    peak: [f64; AUDIO_CHANNELS],
    /// Largest SAMPLE magnitude per channel — gates the expensive interpolation. Per channel
    /// (render-loudness-norms): a gate shared by the channels never let a much quieter channel
    /// through, and its own true peak read as silence.
    sample_peak: [f32; AUDIO_CHANNELS],
    /// How many frames to keep computing interpolation after a “large” sample, per channel.
    tp_countdown: [usize; AUDIO_CHANNELS],
    /// Tail for polyphase true-peak oversampling: a DOUBLED ring per channel (each sample written
    /// at `pos` and `pos + TP_TAPS`), so the last `TP_TAPS` samples, oldest first, are the slice
    /// `[pos, pos + TP_TAPS)` — no rotate of the tail per sample (kernel-specialization).
    tp_ring: [[f32; 2 * TP_TAPS]; AUDIO_CHANNELS],
    tp_pos: [usize; AUDIO_CHANNELS],
}

/// Length of the true-peak interpolator kernel per phase.
const TP_TAPS: usize = 8;
/// Oversampling ×4 — the minimum required by BS.1770-4 for 48 kHz.
const TP_OVER: usize = 4;
/// Fraction of the largest sample from which interpolation is computed.
/// 0.7 ≈ −3 dB: the inter-sample overshoot of a band-limited signal does not exceed
/// the neighboring sample by more than about 1.5 dB, so the margin is threefold.
const TP_GATE: f32 = 0.7;

impl Default for LoudnessMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoudnessMeter {
    pub fn new() -> LoudnessMeter {
        let sr = kt_time::AUDIO_SAMPLE_RATE as usize;
        LoudnessMeter {
            filters: [k_filter(); AUDIO_CHANNELS],
            step_sum: [0.0; AUDIO_CHANNELS],
            step_frames: 0,
            frames_per_step: sr * STEP_MS / 1000,
            steps: std::collections::VecDeque::new(),
            blocks: Vec::new(),
            blocks_ch: Vec::new(),
            short_steps: std::collections::VecDeque::new(),
            short_blocks: Vec::new(),
            peak: [0.0; AUDIO_CHANNELS],
            sample_peak: [0.0; AUDIO_CHANNELS],
            tp_countdown: [0; AUDIO_CHANNELS],
            tp_ring: [[0.0; 2 * TP_TAPS]; AUDIO_CHANNELS],
            tp_pos: [0; AUDIO_CHANNELS],
        }
    }

    /// Feed a piece of interleaved audio (f32, 48 kHz, 2 channels — invariant 1).
    ///
    /// At the CPU's level (`kt_plugin_api::fast!`, the same bytes): the export pass and every first
    /// opening of a file run it over the whole material.
    pub fn feed(&mut self, data: &[f32]) {
        kt_plugin_api::fast!(self.feed_at_level(data))
    }

    #[inline(always)]
    fn feed_at_level(&mut self, data: &[f32]) {
        let table = tp_table();
        for frame in data.chunks_exact(AUDIO_CHANNELS) {
            for (ch, &x) in frame.iter().enumerate() {
                let (shelf, hp) = &mut self.filters[ch];
                let y = hp.step(shelf.step(x as f64));
                self.step_sum[ch] += y * y;
                self.true_peak(ch, x, table);
            }
            self.step_frames += 1;
            if self.step_frames >= self.frames_per_step {
                self.close_step();
            }
        }
    }

    fn close_step(&mut self) {
        let n = self.step_frames.max(1) as f64;
        let mut mean = [0.0f64; AUDIO_CHANNELS];
        for (m, sum) in mean.iter_mut().zip(self.step_sum.iter_mut()) {
            *m = *sum / n;
            *sum = 0.0;
        }
        self.step_frames = 0;

        // 400 ms block = 4 steps
        self.steps.push_back(mean);
        while self.steps.len() > BLOCK_MS / STEP_MS {
            self.steps.pop_front();
        }
        if self.steps.len() == BLOCK_MS / STEP_MS {
            self.blocks.push(block_power(&self.steps));
            let mut ch = [0.0f64; AUDIO_CHANNELS];
            for (c, v) in ch.iter_mut().enumerate() {
                *v = self.steps.iter().map(|s| s[c]).sum::<f64>() / self.steps.len() as f64;
            }
            self.blocks_ch.push(ch);
        }
        // short block of 3 s — for LRA
        self.short_steps.push_back(mean);
        while self.short_steps.len() > 3000 / STEP_MS {
            self.short_steps.pop_front();
        }
        if self.short_steps.len() == 3000 / STEP_MS {
            self.short_blocks.push(block_power(&self.short_steps));
        }
    }

    /// True peak: the signal between samples can be higher than in them.
    ///
    /// Interpolation is NOT computed on every frame. The inter-sample overshoot lives
    /// next to large samples — that is what “peak” means; in silence it cannot
    /// be present. Therefore the polyphase sum (eight taps over four phases, that is
    /// 64 multiplications per frame) is run only around samples comparable to the
    /// largest, and keeps the kernel length after it, so that an overshoot to the RIGHT
    /// of a large sample also gets counted.
    ///
    /// Without the gate the pass over a long file ran into this sum: indexing the material
    /// slowed several times over, and it runs on every project open.
    #[inline(always)]
    fn true_peak(&mut self, ch: usize, x: f32, table: &[[f64; TP_OVER]; TP_TAPS]) {
        let pos = self.tp_pos[ch];
        self.tp_ring[ch][pos] = x;
        self.tp_ring[ch][pos + TP_TAPS] = x;
        self.tp_pos[ch] = (pos + 1) % TP_TAPS;
        let a = x.abs();
        if a > self.sample_peak[ch] {
            self.sample_peak[ch] = a;
        }
        if a >= self.sample_peak[ch] * TP_GATE {
            self.tp_countdown[ch] = TP_TAPS;
        }
        if self.tp_countdown[ch] == 0 {
            return;
        }
        self.tp_countdown[ch] -= 1;
        let from = self.tp_pos[ch];
        let tail = &self.tp_ring[ch][from..from + TP_TAPS];
        for phase in 0..TP_OVER {
            let mut acc = 0.0f64;
            for (&s, k) in tail.iter().zip(table) {
                acc += s as f64 * k[phase];
            }
            self.peak[ch] = self.peak[ch].max(acc.abs());
        }
    }

    /// Close the measurement. The tail shorter than a step is appended: discarding it
    /// would mean missing a peak in the last tenth of a second.
    pub fn finish(mut self) -> Loudness {
        if self.step_frames > 0 {
            self.close_step();
        }
        let to_db = |p: f64| if p > 0.0 { 20.0 * p.flog10() } else { -f64::INFINITY };
        let dbtp = to_db(self.peak.iter().copied().fold(0.0, f64::max));
        let lufs = gated_lufs(&self.blocks);
        let lra = loudness_range(&self.short_blocks);
        let kept = gated_blocks(&self.blocks);
        let channels = std::array::from_fn(|c| {
            let mean = if kept.is_empty() {
                0.0
            } else {
                kept.iter().map(|&i| self.blocks_ch[i][c]).sum::<f64>() / kept.len() as f64
            };
            let l = power_to_lufs(mean);
            ChannelLoudness { lufs: l.is_finite().then_some(l), dbtp: to_db(self.peak[c]) }
        });
        Loudness { lufs, lra, dbtp, channels }
    }
}

fn block_power(steps: &std::collections::VecDeque<[f64; AUDIO_CHANNELS]>) -> f64 {
    let mut sum = 0.0;
    for ch in 0..AUDIO_CHANNELS {
        let mean: f64 = steps.iter().map(|s| s[ch]).sum::<f64>() / steps.len() as f64;
        sum += CH_WEIGHT[ch] * mean;
    }
    sum
}

fn power_to_lufs(p: f64) -> f64 {
    if p <= 0.0 { -f64::INFINITY } else { -0.691 + 10.0 * p.flog10() }
}

/// BS.1770 two-pass gate: absolute −70 LUFS, then relative
/// −10 LU from what remains.
fn gated_lufs(blocks: &[f64]) -> Option<f64> {
    let pass1: Vec<f64> =
        blocks.iter().copied().filter(|p| power_to_lufs(*p) > ABS_GATE_LUFS).collect();
    if pass1.is_empty() {
        return None;
    }
    let prelim = power_to_lufs(pass1.iter().sum::<f64>() / pass1.len() as f64);
    let gate = prelim + REL_GATE_LU;
    let pass2: Vec<f64> = pass1.into_iter().filter(|p| power_to_lufs(*p) > gate).collect();
    if pass2.is_empty() {
        return None;
    }
    Some(power_to_lufs(pass2.iter().sum::<f64>() / pass2.len() as f64))
}

/// The indices of the blocks both BS.1770 gates keep (the same selection as `gated_lufs`).
fn gated_blocks(blocks: &[f64]) -> Vec<usize> {
    let pass1: Vec<usize> = (0..blocks.len()).filter(|&i| power_to_lufs(blocks[i]) > ABS_GATE_LUFS).collect();
    if pass1.is_empty() {
        return pass1;
    }
    let prelim = power_to_lufs(pass1.iter().map(|&i| blocks[i]).sum::<f64>() / pass1.len() as f64);
    let gate = prelim + REL_GATE_LU;
    pass1.into_iter().filter(|&i| power_to_lufs(blocks[i]) > gate).collect()
}

/// LRA (EBU Tech 3342): the difference between the 95th and 10th percentiles of short blocks
/// that passed the absolute −70 and relative −20 LU gates.
fn loudness_range(short: &[f64]) -> f64 {
    let pass1: Vec<f64> =
        short.iter().copied().filter(|p| power_to_lufs(*p) > ABS_GATE_LUFS).collect();
    if pass1.len() < 2 {
        return 0.0;
    }
    let prelim = power_to_lufs(pass1.iter().sum::<f64>() / pass1.len() as f64);
    let gate = prelim - 20.0;
    let mut vals: Vec<f64> = pass1.into_iter().map(power_to_lufs).filter(|l| *l > gate).collect();
    if vals.len() < 2 {
        return 0.0;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| -> f64 {
        let i = ((vals.len() - 1) as f64 * q).round() as usize;
        vals[i]
    };
    (at(0.95) - at(0.10)).max(0.0)
}

/// Polyphase interpolator kernel: Hann window on sinc. Computed once per
/// (tap, phase) — the table is small, and the hot path has eight multiplications anyway. Fetched
/// once per `feed`, not per tap.
fn tp_table() -> &'static [[f64; TP_OVER]; TP_TAPS] {
    static TABLE: std::sync::OnceLock<[[f64; TP_OVER]; TP_TAPS]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [[0.0f64; TP_OVER]; TP_TAPS];
        let center = (TP_TAPS as f64 - 1.0) / 2.0;
        for (i, row) in t.iter_mut().enumerate() {
            for (p, v) in row.iter_mut().enumerate() {
                let x = i as f64 - center - p as f64 / TP_OVER as f64;
                let s = if x.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).fsin() / (std::f64::consts::PI * x)
                };
                // Hann window over the kernel length — without it the side lobes
                // add to the peak something that is not in the signal
                let w = 0.5
                    - 0.5 * (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / TP_TAPS as f64).fcos();
                *v = s * w;
            }
        }
        // normalization per phase: a constant signal must give itself back
        for p in 0..TP_OVER {
            let sum: f64 = (0..TP_TAPS).map(|i| t[i][p]).sum();
            if sum.abs() > 1e-9 {
                for row in t.iter_mut() {
                    row[p] /= sum;
                }
            }
        }
        t
    })
}

/// One-off measurement of a finished buffer.
pub fn measure(data: &[f32]) -> Loudness {
    let mut m = LoudnessMeter::new();
    m.feed(data);
    m.finish()
}
