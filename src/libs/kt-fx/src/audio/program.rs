//! The program stage (loudness-norms D3): a gain and a TRUE-PEAK limiter at the project's ceiling, at
//! the output of the mix. Its detector is the meter's own interpolator (`loudness::tp_table`), so the
//! limiter holds what the report will measure.
//!
//! The reduction a moment asks is the smallest one within the look-ahead, held over the
//! interpolator's length on both sides, released over 50 ms and smoothed by a box of the look-ahead:
//! the box ends on the held value when the peak arrives, so the gain is flat under a peak and its
//! attack is a ramp of 1.5 ms. The inner ceiling stands 0.1 dB under the project's for what the gain's
//! own modulation adds between samples.
//!
//! Every frame is taken alone, so the slicing of the calls changes nothing; a signal under the inner
//! ceiling at 0 dB passes to the bit. No allocation after `new`.

use super::loudness::{TP_OVER, TP_TAPS, tp_table};
use kt_plugin_api::math::FrozenMath;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

/// The box's length: the attack.
const BOX: usize = 72;
/// Frames the stage is fed ahead of what it gives: the box and the interpolator's length, so a peak
/// found at the interpolator's far end is held for every sample it was computed from.
pub const LOOKAHEAD: usize = BOX + TP_TAPS;
/// The held maximum: the box and the interpolator's length on both sides of a peak.
const HOLD: usize = BOX + 2 * TP_TAPS;
const RELEASE_S: f64 = 0.050;
/// The gain's own change is spread over this many frames (20 ms).
const GLIDE: u32 = AUDIO_SAMPLE_RATE / 50;
/// Under the project's ceiling, for the gain's own modulation.
pub const INNER_DB: f64 = 0.1;
/// A reduction this small is none: the tail of the release ends on exactly 1.
const NONE: f64 = 1e-7;

fn lin(db: f64) -> f64 {
    10f64.fpowf(db / 20.0)
}

/// What the stage does: the gain and the ceiling it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub gain_db: f64,
    pub ceiling_dbtp: f64,
}

impl Params {
    /// No gain, nothing held: a stage taken away while it plays keeps its delay and changes nothing.
    pub const PASS: Params = Params { gain_db: 0.0, ceiling_dbtp: f64::INFINITY };

    /// Without the look-ahead (a reverse play, a paused level): the gain and a clamp at the ceiling.
    pub fn clamp(&self, samples: &mut [f32]) {
        let (g, c) = (lin(self.gain_db) as f32, lin(self.ceiling_dbtp).min(f32::MAX as f64) as f32);
        for s in samples {
            *s = (*s * g).clamp(-c, c);
        }
    }
}

#[derive(Clone)]
pub struct Stage {
    gain: f32,
    to: f32,
    step: f32,
    glide: u32,
    inner: f64,
    release: f64,
    /// The delayed signal after the gain, and the same for the interpolator (a doubled ring each).
    delay: [[f32; LOOKAHEAD]; AUDIO_CHANNELS],
    taps: [[f32; 2 * TP_TAPS]; AUDIO_CHANNELS],
    tap_pos: usize,
    pos: usize,
    /// The reductions asked (1 − gain) over the hold, and their running maximum's age.
    asked: [f64; HOLD],
    ask_pos: usize,
    max: f64,
    max_age: usize,
    held: f64,
    boxed: [f64; BOX],
    box_pos: usize,
    sum: f64,
    /// Entries of the box that reduce: at none the sum is exactly 0.
    live: usize,
    /// The deepest reduction since the last `take_deepest_db` (1 − gain).
    deepest: f64,
}

impl Stage {
    pub fn new(p: Params) -> Stage {
        // what `frame` skips the interpolator by
        debug_assert!((0..TP_OVER).all(|q| tp_table().iter().map(|r| r[q].abs()).sum::<f64>() < 2.0));
        let gain = lin(p.gain_db) as f32;
        Stage {
            gain,
            to: gain,
            step: 0.0,
            glide: 0,
            inner: lin(p.ceiling_dbtp - INNER_DB),
            release: (-1.0 / (RELEASE_S * AUDIO_SAMPLE_RATE as f64)).fexp(),
            delay: [[0.0; LOOKAHEAD]; AUDIO_CHANNELS],
            taps: [[0.0; 2 * TP_TAPS]; AUDIO_CHANNELS],
            tap_pos: 0,
            pos: 0,
            asked: [0.0; HOLD],
            ask_pos: 0,
            max: 0.0,
            max_age: 0,
            held: 0.0,
            boxed: [0.0; BOX],
            box_pos: 0,
            sum: 0.0,
            live: 0,
            deepest: 0.0,
        }
    }

    /// Another gain or ceiling while it plays: the gain glides, the ceiling holds from here on.
    pub fn set(&mut self, p: Params) {
        self.inner = lin(p.ceiling_dbtp - INNER_DB);
        let to = lin(p.gain_db) as f32;
        if to != self.to {
            self.to = to;
            self.glide = GLIDE;
            self.step = (to - self.gain) / GLIDE as f32;
        }
    }

    /// Forget the signal (a seek): the gain stands at its value at once.
    pub fn reset(&mut self) {
        let (to, inner) = (self.to, self.inner);
        *self = Stage::new(Params::PASS);
        (self.gain, self.to, self.inner) = (to, to, inner);
    }

    /// The deepest reduction since it was last asked, dB (0 — none).
    pub fn take_deepest_db(&mut self) -> f64 {
        let d = std::mem::take(&mut self.deepest);
        if d > 0.0 { -20.0 * (1.0 - d).max(1e-9).flog10() } else { 0.0 }
    }

    /// Feed the frames ahead of the head (`LOOKAHEAD` of them after a reset): nothing is given.
    pub fn prime(&mut self, ahead: &[f32]) {
        kt_plugin_api::fast!({
            for f in ahead.chunks_exact(AUDIO_CHANNELS) {
                self.frame([f[0], f[1]]);
            }
        })
    }

    /// In place: the mix `LOOKAHEAD` ahead goes in, the program's output comes out.
    pub fn process(&mut self, buf: &mut [f32]) {
        kt_plugin_api::fast!({
            for f in buf.chunks_exact_mut(AUDIO_CHANNELS) {
                let out = self.frame([f[0], f[1]]);
                f.copy_from_slice(&out);
            }
        })
    }

    #[inline(always)]
    fn frame(&mut self, x: [f32; AUDIO_CHANNELS]) -> [f32; AUDIO_CHANNELS] {
        if self.glide > 0 {
            self.glide -= 1;
            self.gain = if self.glide == 0 { self.to } else { self.gain + self.step };
        }
        let at = self.tap_pos;
        let mut out = [0f32; AUDIO_CHANNELS];
        let mut near = 0f32;
        for c in 0..AUDIO_CHANNELS {
            let v = x[c] * self.gain;
            self.taps[c][at] = v;
            self.taps[c][at + TP_TAPS] = v;
            out[c] = std::mem::replace(&mut self.delay[c][self.pos], v);
            for t in &self.taps[c][..TP_TAPS] {
                near = near.max(t.abs());
            }
        }
        self.pos = if self.pos + 1 == LOOKAHEAD { 0 } else { self.pos + 1 };
        self.tap_pos = if at + 1 == TP_TAPS { 0 } else { at + 1 };
        // no point between these samples can pass the ceiling: the interpolator's rows sum below 2
        let mut ask = 0.0;
        if near as f64 * 2.0 > self.inner {
            let peak = self.peak().max(near as f64);
            if peak > self.inner {
                ask = 1.0 - self.inner / peak;
            }
        }
        // the largest reduction asked within the hold
        self.asked[self.ask_pos] = ask;
        self.ask_pos = if self.ask_pos + 1 == HOLD { 0 } else { self.ask_pos + 1 };
        self.max_age += 1;
        if ask >= self.max {
            (self.max, self.max_age) = (ask, 0);
        } else if self.max_age >= HOLD {
            // the former maximum left the hold: the next one, and how long it has stood
            let (mut m, mut age) = (0.0, 0);
            for k in 0..HOLD {
                let v = self.asked[(self.ask_pos + k) % HOLD];
                if v >= m {
                    (m, age) = (v, HOLD - 1 - k);
                }
            }
            (self.max, self.max_age) = (m, age);
        }
        self.held = if self.max >= self.held { self.max } else { (self.held * self.release).max(self.max) };
        if self.held < NONE {
            self.held = 0.0;
        }
        // the box over the held reduction
        let was = std::mem::replace(&mut self.boxed[self.box_pos], self.held);
        self.box_pos = if self.box_pos + 1 == BOX { 0 } else { self.box_pos + 1 };
        self.live += usize::from(self.held > 0.0);
        self.live -= usize::from(was > 0.0);
        self.sum = if self.live == 0 { 0.0 } else { self.sum + self.held - was };
        if self.box_pos == 0 && self.live > 0 {
            // the running sum's rounding does not build up
            self.sum = self.boxed.iter().sum();
        }
        if self.sum > 0.0 {
            let cut = (self.sum / BOX as f64).min(1.0);
            self.deepest = self.deepest.max(cut);
            let g = 1.0 - cut;
            for v in &mut out {
                *v = (*v as f64 * g) as f32;
            }
        }
        out
    }

    /// The largest magnitude of the wave between the last samples, as the meter computes it.
    #[inline(always)]
    fn peak(&self) -> f64 {
        let table = tp_table();
        let mut peak = 0.0f64;
        for c in 0..AUDIO_CHANNELS {
            let tail = &self.taps[c][self.tap_pos..self.tap_pos + TP_TAPS];
            for phase in 0..TP_OVER {
                let mut acc = 0.0f64;
                for (&s, k) in tail.iter().zip(table) {
                    acc += s as f64 * k[phase];
                }
                peak = peak.max(acc.abs());
            }
        }
        peak
    }
}
