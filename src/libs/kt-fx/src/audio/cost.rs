//! What an audio chain costs against real time (audio-fx-cost): ONE text for `kt-cli fx-cost` and
//! the cost guard t1980. A cost is never judged by a wall-clock limit: it is a RATIO to a fixed
//! reference kernel measured in the same run, block by block alternately, each block the minimum of
//! several rounds — a busy machine slows both alike, so the ratio holds where a millisecond figure
//! does not.

use std::time::Instant;

use kt_model::FxInstance;

use super::{ChainOpts, TrackProcessor};

const CH: usize = 2;
/// The block the chain is fed in: the engine's chunk (`ClipProcessor::AUTOMATION_STEP_FRAMES`).
pub const BLOCK: usize = 1024;

/// The stimulus: a deterministic stereo mix of a triangle tone and noise at about −12 dBFS,
/// different in the two channels — filters have a top to cut, stereo rows an S part, dynamics a
/// level above their default thresholds. Plain arithmetic, no libm.
pub fn stimulus(frames: usize) -> Vec<f32> {
    let mut out = vec![0f32; frames * CH];
    let mut s: u32 = 0x1234_5678;
    let mut next = || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5
    };
    for i in 0..frames {
        let ph = (i as f32 * 440.0 / 48_000.0).fract();
        let tone = 1.0 - 4.0 * (ph - 0.5).abs();
        let (n1, n2) = (next(), next());
        out[i * CH] = 0.2 * tone + 0.15 * n1;
        out[i * CH + 1] = 0.15 * tone + 0.2 * n2;
    }
    out
}

/// The reference kernel: eight cascaded biquads per channel (transposed direct form II, `f32`,
/// fixed coefficients) — a plain recursive filter bank, the kind of work most audio effects do,
/// as one sequence of dependent multiply-adds. Its cost is the unit the ratios are counted in.
pub struct Reference {
    z: [[[f32; 2]; 8]; CH],
}

impl Default for Reference {
    fn default() -> Reference {
        Reference { z: [[[0.0; 2]; 8]; CH] }
    }
}

impl Reference {
    // a 2 kHz RBJ lowpass, Q 0.707, at 48 kHz
    const B: [f32; 3] = [0.015_466_64, 0.030_933_28, 0.015_466_64];
    const A: [f32; 2] = [-1.617_157_7, 0.679_024_3];

    pub fn process(&mut self, buf: &mut [f32]) {
        for f in buf.chunks_exact_mut(CH) {
            for (c, x) in f.iter_mut().enumerate() {
                let mut v = *x;
                for z in &mut self.z[c] {
                    let y = Self::B[0] * v + z[0];
                    z[0] = Self::B[1] * v - Self::A[0] * y + z[1];
                    z[1] = Self::B[2] * v - Self::A[1] * y;
                    v = y;
                }
                *x = v;
            }
        }
    }
}

/// One chain's cost: its share of real time (seconds of CPU per second of audio) and the reference
/// kernel's, measured together (`chain_cost`).
#[derive(Clone, Copy, Debug)]
pub struct Cost {
    pub share: f64,
    pub reference: f64,
}

impl Cost {
    /// The chain's cost in units of the reference kernel.
    pub fn ratio(&self) -> f64 {
        self.share / self.reference
    }
}

/// The cost of the node chain `fx` on `input`, against the reference kernel.
///
/// Immune to a busy machine by construction, not by a wide bound: the two are timed BLOCK BY BLOCK,
/// alternately (which goes first swaps every block), so each pair runs within microseconds on one
/// core at one clock; each block's time is the minimum over `runs` rounds (a fresh processor and a
/// fresh reference per round, so block `b` does the same work in every round), which drops a
/// preemption or a migration that hit one round. The first block (the state's buffers coming into
/// memory) is not counted: the cost is the running one.
pub fn chain_cost(fx: &[FxInstance], input: &[f32], runs: usize) -> Cost {
    let frames = input.len() / CH;
    let blocks = frames.div_ceil(BLOCK);
    let mut best_k = vec![u64::MAX; blocks];
    let mut best_r = vec![u64::MAX; blocks];
    let (mut bk, mut br) = (vec![0f32; BLOCK * CH], vec![0f32; BLOCK * CH]);
    for _ in 0..runs.max(1) {
        let mut r = Reference::default();
        let mut p = TrackProcessor::new(fx, ChainOpts { fps: 25, ..Default::default() });
        for b in 0..blocks {
            let pos = b * BLOCK;
            let n = BLOCK.min(frames - pos);
            let src = &input[pos * CH..(pos + n) * CH];
            br[..n * CH].copy_from_slice(src);
            bk[..n * CH].copy_from_slice(src);
            let mut time_ref = || {
                let t = Instant::now();
                r.process(&mut br[..n * CH]);
                t.elapsed().as_nanos() as u64
            };
            let (tr, tk) = if b % 2 == 0 {
                let tr = time_ref();
                let t = Instant::now();
                p.process(pos as i64, &mut bk[..n * CH]);
                (tr, t.elapsed().as_nanos() as u64)
            } else {
                let t = Instant::now();
                p.process(pos as i64, &mut bk[..n * CH]);
                let tk = t.elapsed().as_nanos() as u64;
                (time_ref(), tk)
            };
            best_r[b] = best_r[b].min(tr);
            best_k[b] = best_k[b].min(tk);
        }
        std::hint::black_box((&bk, &br));
    }
    let counted = (frames - BLOCK.min(frames)).max(1) as f64 / 48_000.0;
    let sum = |v: &[u64]| v.iter().skip(1).sum::<u64>() as f64 * 1e-9;
    Cost { share: sum(&best_k) / counted, reference: sum(&best_r) / counted }
}

/// The node chain's output on `input` (block by block, as `chain_cost` runs it).
pub fn chain_output(fx: &[FxInstance], input: &[f32]) -> Vec<f32> {
    let mut p = TrackProcessor::new(fx, ChainOpts { fps: 25, ..Default::default() });
    let mut out = input.to_vec();
    for (i, b) in out.chunks_mut(BLOCK * CH).enumerate() {
        p.process((i * BLOCK) as i64, b);
    }
    out
}

/// The node chain's output on `input` with its values changed to `then` halfway (a person turning
/// a choice during playback: `apply_params`, not a rebuild — the state lives on).
pub fn switched_output(first: &FxInstance, then: &FxInstance, input: &[f32]) -> Vec<f32> {
    let mut p = TrackProcessor::new(std::slice::from_ref(first), ChainOpts { fps: 25, ..Default::default() });
    let mut out = input.to_vec();
    let half = out.len() / (BLOCK * CH) / 2;
    for (i, b) in out.chunks_mut(BLOCK * CH).enumerate() {
        if i == half {
            p.apply_params(std::slice::from_ref(then), false);
        }
        p.process((i * BLOCK) as i64, b);
    }
    out
}

/// The switches t1981 holds: (name, kind, the values at the start, the values from halfway).
pub fn switches() -> Vec<(&'static str, FxInstance, FxInstance)> {
    let Some(vhs) = FxInstance::all().iter().find(|f| f.tag() == "vhs_tape_sound") else { return Vec::new() };
    let with = |sets: &[(&str, f64)]| {
        let mut f = vhs.clone();
        for (k, v) in sets {
            assert!(f.set_param(k, *v), "vhs_tape_sound has no row {k}");
        }
        f
    };
    vec![
        // Hi-Fi alone, then the linear track (its ring was recorded unshaped), at another bias
        ("vhs hifi->normal", with(&[]), with(&[("audio_out", 1.0), ("bias", 0.6), ("dropouts_per_s", 200.0)])),
        // the linear track in stereo with Dolby, then Hi-Fi alone, then back is not needed: mix -> hifi
        ("vhs mix->hifi", with(&[("audio_out", 2.0), ("linear_tracks", 1.0), ("dolby", 1.0), ("speed", 2.0), ("dropout_us", 60.0), ("dropouts_per_s", 300.0)]), with(&[])),
        ("vhs hifi->mix stereo", with(&[("speed", 1.0)]), with(&[("audio_out", 2.0), ("linear_tracks", 1.0), ("speed", 1.0), ("dropouts_per_s", 300.0), ("dropout_us", 30.0)])),
    ]
}

/// A digest of an output's bits (FNV-1a over the `f32` words): t1981's table holds them.
pub fn digest(out: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in out {
        for b in v.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The heavy settings of a kind: the rows that make it do the most work, as a person can set them
/// (`None` — its defaults are already its heaviest work, or its rows do not change the work).
pub fn heavy(fx: &FxInstance) -> Option<FxInstance> {
    let set: &[(&str, f64)] = match fx.tag() {
        "slowed_reverb" => &[("mix", 1.0), ("decay", 0.98), ("predelay_ms", 200.0)],
        "phaser" => &[("stages", 12.0), ("feedback", 0.9)],
        "flanger" => &[("feedback", 0.9)],
        "datamosh_sound" => &[("audio_predict", 1.0), ("loss", 0.5)],
        "delay" => &[("time_ms", 2000.0), ("feedback", 0.95)],
        "chorus" => &[("voices", 4.0)],
        "pitch_shift" => &[("semitones", -12.0)],
        "compressor" => &[("attack_ms", 0.1), ("threshold_db", -40.0)],
        "limiter" => &[("lookahead_ms", 20.0)],
        "eq" => &[("low_gain_db", 12.0), ("mid_gain_db", 12.0), ("high_gain_db", 12.0)],
        "vhs_tape_sound" => &[("decks", 1.0), ("audio_out", 2.0), ("dropouts_per_s", 200.0), ("dropout_us", 60.0), ("speed", 2.0)],
        "camera_isp_sound" => &[("mic_scene_spl", 120.0), ("mic_wind", 1.0), ("mic_channels", 1.0)],
        _ => return None,
    };
    let mut f = fx.clone();
    let mut any = false;
    for (k, v) in set {
        any |= f.set_param(k, *v);
    }
    any.then_some(f)
}
