//! Findings of the finished sound at their times (loudness-norms D5): where it passes the ceiling, where
//! one channel is silent, where one is far louder than the other. Fed with what is written, in any
//! slicing; bounded.

use super::loudness::{TP_OVER, TP_TAPS, tp_table};
use kt_plugin_api::math::FrozenMath;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFinding {
    /// The true peak over the project's ceiling.
    OverCeiling,
    /// One channel (0 — left) silent while the other sounds.
    Silent(usize),
    /// One channel (0 — left) louder than the other by more than `IMBALANCE_DB`.
    Louder(usize),
}

/// From the range's start, in 48 kHz frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundSpan {
    pub what: SoundFinding,
    pub from: u64,
    pub to: u64,
    /// The finding's own number: dBTP, or the difference in dB (0 for silence).
    pub value: f64,
}

/// A window of the levels: 100 ms.
const STEP: usize = AUDIO_SAMPLE_RATE as usize / 10;
/// A shorter silence or imbalance is music, not a finding.
const MIN_STEPS: u64 = 10;
/// A gap this short inside one finding does not split it.
const BRIDGE_STEPS: u64 = 3;
const IMBALANCE_DB: f64 = 6.0;
const SILENT_DB: f64 = -60.0;
const SOUNDS_DB: f64 = -50.0;
/// Findings kept of one sound.
const KEPT: usize = 200;

#[derive(Clone, Copy)]
struct Open {
    span: SoundSpan,
    /// Steps of the span, the gaps inside not counted.
    steps: u64,
}

pub struct Finder {
    ceiling: f64,
    sum: [f64; AUDIO_CHANNELS],
    peak: f64,
    n: usize,
    frames: u64,
    ring: [[f32; 2 * TP_TAPS]; AUDIO_CHANNELS],
    pos: usize,
    /// Over the ceiling, silent L/R, louder L/R.
    open: [Option<Open>; 5],
    found: Vec<SoundSpan>,
}

impl Finder {
    pub fn new(ceiling_dbtp: f64) -> Finder {
        Finder {
            ceiling: 10f64.fpowf(ceiling_dbtp / 20.0),
            sum: [0.0; AUDIO_CHANNELS],
            peak: 0.0,
            n: 0,
            frames: 0,
            ring: [[0.0; 2 * TP_TAPS]; AUDIO_CHANNELS],
            pos: 0,
            open: [None; 5],
            found: Vec::new(),
        }
    }

    pub fn feed(&mut self, data: &[f32]) {
        kt_plugin_api::fast!(self.feed_at_level(data))
    }

    #[inline(always)]
    fn feed_at_level(&mut self, data: &[f32]) {
        let table = tp_table();
        for f in data.chunks_exact(AUDIO_CHANNELS) {
            let at = self.pos;
            self.pos = if at + 1 == TP_TAPS { 0 } else { at + 1 };
            for c in 0..AUDIO_CHANNELS {
                let x = f[c];
                self.sum[c] += x as f64 * x as f64;
                self.ring[c][at] = x;
                self.ring[c][at + TP_TAPS] = x;
                let tail = &self.ring[c][self.pos..self.pos + TP_TAPS];
                let near = tail.iter().fold(0f32, |m, v| m.max(v.abs())) as f64;
                // the interpolator's rows sum below 2: nothing here reaches the ceiling
                if near * 2.0 > self.ceiling {
                    for phase in 0..TP_OVER {
                        let mut acc = 0.0f64;
                        for (&s, k) in tail.iter().zip(table) {
                            acc += s as f64 * k[phase];
                        }
                        self.peak = self.peak.max(acc.abs());
                    }
                }
            }
            self.n += 1;
            if self.n == STEP {
                self.step();
            }
        }
    }

    fn step(&mut self) {
        let db = |p: f64| if p > 0.0 { 10.0 * p.flog10() } else { -f64::INFINITY };
        let level: [f64; AUDIO_CHANNELS] = std::array::from_fn(|c| db(self.sum[c] / self.n as f64));
        let (from, to) = (self.frames, self.frames + self.n as u64);
        self.frames = to;
        let peak_db = 2.0 * db(self.peak);
        let over = (self.peak > self.ceiling).then_some(peak_db);
        self.mark(0, SoundFinding::OverCeiling, over, from, to);
        for c in 0..AUDIO_CHANNELS {
            let other = level[1 - c];
            let silent = level[c] < SILENT_DB && other > SOUNDS_DB;
            self.mark(1 + c, SoundFinding::Silent(c), silent.then_some(0.0), from, to);
            let apart = level[c] - other;
            let louder = level[c] > SOUNDS_DB && other >= SILENT_DB && apart > IMBALANCE_DB;
            self.mark(3 + c, SoundFinding::Louder(c), louder.then_some(apart), from, to);
        }
        self.sum = [0.0; AUDIO_CHANNELS];
        self.peak = 0.0;
        self.n = 0;
    }

    fn mark(&mut self, k: usize, what: SoundFinding, value: Option<f64>, from: u64, to: u64) {
        match (value, &mut self.open[k]) {
            (Some(v), Some(o)) => {
                o.span.to = to;
                o.span.value = o.span.value.max(v);
                o.steps += 1;
            }
            (Some(v), None) => self.open[k] = Some(Open { span: SoundSpan { what, from, to, value: v }, steps: 1 }),
            (None, Some(o)) if from >= o.span.to + BRIDGE_STEPS * STEP as u64 => self.close(k),
            _ => {}
        }
    }

    fn close(&mut self, k: usize) {
        let Some(o) = self.open[k].take() else { return };
        if (k == 0 || o.steps >= MIN_STEPS) && self.found.len() < KEPT {
            self.found.push(o.span);
        }
    }

    /// The findings by their start.
    pub fn finish(mut self) -> Vec<SoundSpan> {
        if self.n > 0 {
            self.step();
        }
        for k in 0..self.open.len() {
            self.close(k);
        }
        self.found.sort_by_key(|s| (s.from, s.to));
        self.found
    }
}
