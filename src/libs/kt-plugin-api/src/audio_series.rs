//! The analysis series an audio-reactive generator listens through (gen-audio-reactive, F-TXT-010).
//!
//! One RECORD per audio burst (1024 frames, 1920 ticks) of a file: the RMS of each channel, the peak,
//! [`BANDS`] log-spaced band levels and [`WAVE`] waveform points, one byte each. The series is a pure
//! function of the file's decoded audio (invariant 11): the analysis reads each burst alone, through the
//! frozen FFT, so a series computed twice is the same bytes, and a store may keep it.
//!
//! A frame reads [`frame_values`]: the levels and bands at its moment, the same HELD (a fold over the
//! last [`HOLD_BURSTS`] records that falls linearly), and the waveform around it. The hold is a fold,
//! not a state: a frame stays a function of its time, so seek and render agree.

use crate::audio_codec::Fft;
use crate::math::FrozenMath;

/// Band levels per record.
pub const BANDS: usize = 16;
/// Waveform points per record (one per 32 frames: the sample of the largest magnitude).
pub const WAVE: usize = 32;
/// Bytes per record: the two channels' RMS, the peak, the bands, the waveform.
pub const REC: usize = 3 + BANDS + WAVE;
/// The analysis' version: a stored series of another version is computed again.
pub const VERSION: u32 = 1;
/// Records a held value falls over, from full to nothing (24 × 21.3 ms ≈ 0.5 s).
pub const HOLD_BURSTS: usize = 24;
/// Frames per burst (the engine's audio burst).
pub const BURST_FRAMES: usize = 1024;
/// Ticks per burst.
pub const BURST_TICKS: i64 = 1920;

/// Where a frame's values lie in [`frame_values`]' output (the storage buffer a kind's shader reads).
pub mod at {
    /// RMS left, RMS right, peak, held level (the louder channel's RMS, held), each 0..1.
    pub const LEVEL: usize = 0;
    /// [`super::BANDS`] band levels now, 0..1.
    pub const BANDS: usize = 4;
    /// The same bands held.
    pub const HELD: usize = BANDS + super::BANDS;
    /// [`WAVE_POINTS`] waveform samples −1..1, the earliest first, ending at the frame's burst.
    pub const WAVE: usize = HELD + super::BANDS;
    /// Waveform points a frame reads: its burst and the one before.
    pub const WAVE_POINTS: usize = 2 * super::WAVE;
    /// The whole length.
    pub const LEN: usize = WAVE + WAVE_POINTS;
}

const LEVEL_FLOOR_DB: f64 = -60.0;
const BAND_FLOOR_DB: f64 = -72.0;
const LOW_HZ: f64 = 40.0;
const HIGH_HZ: f64 = 16_000.0;

/// A whole file's series. Equal series have equal [`AudioSeries::hash`].
#[derive(Clone, PartialEq, Eq)]
pub struct AudioSeries {
    bytes: Box<[u8]>,
    hash: u64,
}

impl std::fmt::Debug for AudioSeries {
    // the bytes are megabytes: the hash names them (`GenState`'s Debug reaches the engine's keys)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AudioSeries({} bursts, {:016x})", self.bursts(), self.hash)
    }
}

impl AudioSeries {
    /// A series from its stored bytes; `None` when they are not whole records.
    pub fn from_bytes(bytes: Vec<u8>) -> Option<AudioSeries> {
        if !bytes.len().is_multiple_of(REC) {
            return None;
        }
        let hash = fnv1a(&bytes);
        Some(AudioSeries { bytes: bytes.into_boxed_slice(), hash })
    }

    /// The bytes a store keeps.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The content hash (FNV-1a 64 of the bytes): what the frame key carries.
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Records (bursts) in the series.
    pub fn bursts(&self) -> usize {
        self.bytes.len() / REC
    }

    /// Record `i`, or `None` outside the series (silence).
    pub fn record(&self, i: i64) -> Option<&[u8]> {
        if i < 0 || i as usize >= self.bursts() {
            return None;
        }
        let s = i as usize * REC;
        Some(&self.bytes[s..s + REC])
    }
}

fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The per-burst analysis: a Hann window, one 1024-point FFT of the burst's mono sum, the bands as the
/// loudest bin between log-spaced edges, the levels and the waveform from the samples.
pub struct Analyzer {
    fft: Fft,
    window: Vec<f64>,
    /// The first bin of each band and one past the last band.
    edges: [usize; BANDS + 1],
    norm: f64,
    re: Vec<f64>,
    im: Vec<f64>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Analyzer::new()
    }
}

impl Analyzer {
    pub fn new() -> Analyzer {
        let n = BURST_FRAMES;
        let window: Vec<f64> =
            (0..n).map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).fcos()).collect();
        // a full-scale sine on a bin reads 0 dB: the window's sum is the amplitude's scale
        let norm = 2.0 / window.iter().sum::<f64>();
        let bin_hz = 48_000.0 / n as f64;
        let mut edges = [0usize; BANDS + 1];
        for (i, e) in edges.iter_mut().enumerate() {
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).fpowf(i as f64 / BANDS as f64);
            *e = ((hz / bin_hz) + 0.5) as usize;
        }
        // every band holds at least one bin (the low bands are narrower than a bin)
        edges[0] = edges[0].max(1);
        for i in 1..=BANDS {
            edges[i] = edges[i].max(edges[i - 1] + 1);
        }
        Analyzer { fft: Fft::new(n), window, edges, norm, re: vec![0.0; n], im: vec![0.0; n] }
    }

    /// One record from a burst: `data` is interleaved stereo, [`BURST_FRAMES`] frames.
    pub fn burst(&mut self, data: &[f32]) -> [u8; REC] {
        let mut out = [0u8; REC];
        let n = BURST_FRAMES;
        let (mut sl, mut sr, mut peak) = (0f64, 0f64, 0f64);
        for i in 0..n {
            let (l, r) = (data.get(2 * i).copied().unwrap_or(0.0) as f64, data.get(2 * i + 1).copied().unwrap_or(0.0) as f64);
            sl += l * l;
            sr += r * r;
            peak = peak.max(l.abs()).max(r.abs());
            self.re[i] = 0.5 * (l + r) * self.window[i];
            self.im[i] = 0.0;
        }
        out[0] = db_byte(((sl / n as f64).sqrt(), LEVEL_FLOOR_DB));
        out[1] = db_byte(((sr / n as f64).sqrt(), LEVEL_FLOOR_DB));
        out[2] = db_byte((peak, LEVEL_FLOOR_DB));
        self.fft.run(&mut self.re, &mut self.im, false);
        for b in 0..BANDS {
            let mut m = 0f64;
            for k in self.edges[b]..self.edges[b + 1].min(n / 2) {
                m = m.max(self.re[k] * self.re[k] + self.im[k] * self.im[k]);
            }
            out[3 + b] = db_byte((m.sqrt() * self.norm, BAND_FLOOR_DB));
        }
        let span = n / WAVE;
        for p in 0..WAVE {
            let mut v = 0f64;
            for i in p * span..(p + 1) * span {
                let s = 0.5 * (data.get(2 * i).copied().unwrap_or(0.0) as f64 + data.get(2 * i + 1).copied().unwrap_or(0.0) as f64);
                if s.abs() > v.abs() {
                    v = s;
                }
            }
            out[3 + BANDS + p] = ((v * 127.0).round().clamp(-127.0, 127.0) as i8) as u8;
        }
        out
    }
}

/// An amplitude as a byte over `floor`..0 dB.
fn db_byte((amp, floor): (f64, f64)) -> u8 {
    if amp <= 0.0 {
        return 0;
    }
    let db = 20.0 * amp.flog10();
    (((db - floor) / -floor).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The values a frame reads at material tick `t` of `series` (layout: [`at`]). Outside the series —
/// silence: zeros.
pub fn frame_values(series: &AudioSeries, t: i64, out: &mut [f32; at::LEN]) {
    out.fill(0.0);
    let b = t.div_euclid(BURST_TICKS);
    let unit = |v: u8| v as f32 / 255.0;
    if let Some(r) = series.record(b) {
        out[at::LEVEL] = unit(r[0]);
        out[at::LEVEL + 1] = unit(r[1]);
        out[at::LEVEL + 2] = unit(r[2]);
        for i in 0..BANDS {
            out[at::BANDS + i] = unit(r[3 + i]);
        }
    }
    // held: the largest of the last records, each fallen by its age
    let mut level = 0f32;
    let mut held = [0f32; BANDS];
    for k in 0..=HOLD_BURSTS {
        let Some(r) = series.record(b - k as i64) else { continue };
        let fall = k as f32 / HOLD_BURSTS as f32;
        level = level.max(unit(r[0].max(r[1])) - fall);
        for i in 0..BANDS {
            held[i] = held[i].max(unit(r[3 + i]) - fall);
        }
    }
    out[at::LEVEL + 3] = level.max(0.0);
    for i in 0..BANDS {
        out[at::HELD + i] = held[i].max(0.0);
    }
    for (j, rb) in [b - 1, b].into_iter().enumerate() {
        if let Some(r) = series.record(rb) {
            for p in 0..WAVE {
                out[at::WAVE + j * WAVE + p] = (r[3 + BANDS + p] as i8) as f32 / 127.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, amp: f64, burst: i64) -> Vec<f32> {
        (0..BURST_FRAMES)
            .flat_map(|i| {
                let n = burst * BURST_FRAMES as i64 + i as i64;
                let v = (amp * (n as f64 * hz * std::f64::consts::TAU / 48_000.0).fsin()) as f32;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn a_sine_lands_in_its_band_and_level() {
        let mut a = Analyzer::new();
        let r = a.burst(&sine(1000.0, 0.5, 3));
        let bands = &r[3..3 + BANDS];
        let top = (0..BANDS).max_by_key(|&i| bands[i]).unwrap();
        let hz = |i: usize| LOW_HZ * (HIGH_HZ / LOW_HZ).fpowf(i as f64 / BANDS as f64);
        assert!(hz(top) <= 1000.0 && 1000.0 < hz(top + 1) * 1.1, "1 kHz in band {top}");
        // RMS of a 0.5 sine is −9 dB: (−9 + 60) / 60 · 255
        assert!((r[0] as i32 - 217).abs() <= 1, "RMS byte {}", r[0]);
        let silent = a.burst(&vec![0.0; 2 * BURST_FRAMES]);
        assert!(silent.iter().all(|&v| v == 0));
    }
}
