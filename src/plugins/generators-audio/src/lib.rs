//! The sound generators (generators-placement-and-audio, F-AUD-014): a tone, white/pink/brown
//! noise, a sine sweep, a beep train and silence. A sound kind has no picture — its source reports
//! `has_video: false`, so creation makes an audio clip only — and fills its burst itself from the
//! GLOBAL sample number (`generator::Synth`).
//!
//! Rules every kernel here keeps (design D4–D10):
//! - **a burst is a pure function of (values, first sample)**: no state between bursts, so a seek,
//!   a straight play and an export give the same bytes (invariant 11). The noise is therefore
//!   counter-based, not a filter;
//! - **every sine is `generator::sine`**, the host's 1 kHz tone's own text: a 1 kHz tone equals
//!   SMPTE's byte for byte (t1155);
//! - **levels are AES17 dBFS**: a sine's level is its peak, a noise's its RMS on the same scale;
//! - transcendental functions only through `kt_plugin_api::math` (t1066).

use std::f64::consts::{LN_10, SQRT_2, TAU};
use std::sync::Arc;

use kt_plugin_api::generator::{PathTok, PathVal, Row, count, db_amp, sine, smooth, sound};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{ParamUnit, Plugin};

/// The creation menu's group.
const AUDIO: &str = "genkind.group_audio";
/// The engine's sample rate (invariant 1).
const RATE: f64 = 48_000.0;

/// Where a mono sample goes: both channels, or one (channel identification at a master's head).
const CHANNELS: &[&str] = &["both", "left", "right"];
const CHANNEL_TOKENS: &[&str] = &["lr", "l", "r"];

#[inline]
fn put(out: &mut [f32], f: usize, v: f32, channels: f64) {
    match channels.round() as i64 {
        1 => out[f * 2] = v,
        2 => out[f * 2 + 1] = v,
        _ => {
            out[f * 2] = v;
            out[f * 2 + 1] = v;
        }
    }
}

// ---- tone ---------------------------------------------------------------------------------------

/// A sine at `freq`, peak `level` dBFS (rows: freq, level, channels).
fn tone(p: &[f64], n0: i64, out: &mut [f32]) {
    let amp = db_amp(p[1]);
    for f in 0..out.len() / 2 {
        let v = sine(n0 + f as i64, p[0]) * amp;
        put(out, f, v as f32, p[2]);
    }
}

// ---- noise --------------------------------------------------------------------------------------

const COLORS: &[&str] = &["white", "pink", "brown"];

/// The rows of a colour and their weights: white is one row per sample; pink — Voss–McCartney,
/// rows held 2^k samples with equal weights, 3 dB/octave down to ~12 Hz (row 11); brown — rows
/// weighted by 2^(k/2), 6 dB/octave down to ~47 Hz (row 9), flat below so it does not wander.
fn rows_of(color: usize) -> (usize, fn(usize) -> f64) {
    match color {
        1 => (12, |_| 1.0),
        2 => (10, |k| (2f64.powi(k as i32)).sqrt()),
        _ => (1, |_| 1.0),
    }
}

/// A uniform value in [−1, 1) for (seed, channel, row, held index): the splitmix64 finaliser,
/// integer only — the counter-based rule of the picture noise, for sound.
#[inline]
fn uni(seed: u64, ch: u64, row: u64, m: u64) -> f64 {
    let mut x = m ^ (row << 48) ^ (ch << 56) ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// Coloured noise at RMS `level` dBFS AES17 (rows: color, level, seed, stereo). Each sample is a
/// pure function of its number: the rows are hash values held 2^k samples.
fn noise(p: &[f64], n0: i64, out: &mut [f32]) {
    let (rows, weight) = rows_of(p[0].round().clamp(0.0, 2.0) as usize);
    let mut w = [0f64; 12];
    let mut var = 0.0;
    for (k, wk) in w.iter_mut().enumerate().take(rows) {
        *wk = weight(k);
        var += *wk * *wk / 3.0; // a uniform [−1, 1) row has variance 1/3
    }
    let scale = db_amp(p[1]) / SQRT_2 / var.sqrt();
    let seed = p[2].round().max(0.0) as u64;
    let stereo = p[3] >= 0.5;
    for f in 0..out.len() / 2 {
        let n = (n0 + f as i64) as u64;
        let at = |ch: u64| {
            let mut s = 0.0;
            for (k, wk) in w.iter().enumerate().take(rows) {
                s += wk * uni(seed, ch, k as u64, n >> k);
            }
            (s * scale).clamp(-1.0, 1.0) as f32
        };
        let l = at(0);
        out[f * 2] = l;
        out[f * 2 + 1] = if stereo { at(1) } else { l };
    }
}

// ---- sweep --------------------------------------------------------------------------------------

const CURVES: &[&str] = &["log", "linear"];
const CURVE_TOKENS: &[&str] = &["log", "lin"];

/// A sine sweep from `freq_start` to `freq_end` over `duration`, repeating (rows: freq_start,
/// freq_end, duration, curve, level, channels). The phase is the INTEGRAL of the frequency: the
/// naive `sin(2π·f(t)·t)` sweeps twice as far.
fn sweep(p: &[f64], n0: i64, out: &mut [f32]) {
    let (f0, f1) = (p[0].max(1.0), p[1].max(1.0));
    let len = (p[2] * RATE).round().max(1.0) as i64;
    let span = len as f64 / RATE;
    let log = p[3] < 0.5;
    let k = f1 / f0;
    // the log sweep's constant: f0·T / ln k (a flat "sweep" degenerates to a tone)
    let lk = k.flog10() * LN_10;
    let flat = lk.abs() < 1e-12;
    let amp = db_amp(p[4]);
    for f in 0..out.len() / 2 {
        let t = (n0 + f as i64).rem_euclid(len) as f64 / RATE;
        let cycles = if flat {
            f0 * t
        } else if log {
            f0 * span / lk * (k.fpowf(t / span) - 1.0)
        } else {
            f0 * t + (f1 - f0) * t * t / (2.0 * span)
        };
        let v = (TAU * (cycles - cycles.floor())).fsin() * amp;
        put(out, f, v as f32, p[5]);
    }
}

// ---- beep ---------------------------------------------------------------------------------------

/// The tone gated on `(n mod period) < length` from the material's start (rows: freq, level,
/// period, length, channels) — the countdown leader's pip as a kind of its own. A hard gate, as the
/// 2-pop is.
fn beep(p: &[f64], n0: i64, out: &mut [f32]) {
    let amp = db_amp(p[1]);
    let period = (p[2] * RATE).round().max(1.0) as i64;
    let length = (p[3] * RATE / 1000.0).round() as i64;
    for f in 0..out.len() / 2 {
        let n = n0 + f as i64;
        if n.rem_euclid(period) < length {
            put(out, f, (sine(n, p[0]) * amp) as f32, p[4]);
        }
    }
}

// ---- silence ------------------------------------------------------------------------------------

/// Silence: the host zeroes the burst; the kind exists to hold a place on an audio track.
fn silence(_: &[f64], _: i64, _: &mut [f32]) {}

// ---- the rows -----------------------------------------------------------------------------------

fn freq(n: &'static str, d: f64) -> Row {
    smooth(n, 20.0, 20_000.0, d, 1.0, ParamUnit::Hz)
}

fn level() -> Row {
    smooth("level", -60.0, 0.0, -20.0, 0.1, ParamUnit::Db)
}

/// The family's kinds, in `BUILTIN_IDS` order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    let ch = || Row::Choice("channels", CHANNELS);
    const CH: PathTok = PathTok { key: "ch", val: PathVal::Choice("channels", CHANNEL_TOKENS) };
    const DB: PathTok = PathTok { key: "db", val: PathVal::Num("level") };
    vec![
        // the line-up tone: 1 kHz at −20 dBFS (SMPTE RP 155's reference)
        Arc::new(sound(
            "tone",
            "genkind.tone",
            AUDIO,
            vec![freq("freq", 1000.0), level(), ch()],
            &[PathTok { key: "f", val: PathVal::Num("freq") }, DB, CH],
            tone,
        )),
        // white/pink/brown noise; `noise` is the picture noise's id
        Arc::new(sound(
            "audio_noise",
            "genkind.audio_noise",
            AUDIO,
            vec![Row::Choice("color", COLORS), level(), count("seed", 0.0, 9999.0, 1.0), Row::Flag("stereo", false)],
            &[
                PathTok { key: "c", val: PathVal::Choice("color", COLORS) },
                DB,
                PathTok { key: "seed", val: PathVal::Int("seed", 0, 9999) },
                PathTok { key: "st", val: PathVal::Flag1("stereo") },
            ],
            noise,
        )),
        // a sine sweep, logarithmic by default (equal time per octave — the measuring one)
        Arc::new(sound(
            "sweep",
            "genkind.sweep",
            AUDIO,
            vec![
                freq("freq_start", 20.0),
                freq("freq_end", 20_000.0),
                smooth("duration", 0.1, 600.0, 10.0, 0.1, ParamUnit::Sec),
                Row::Choice("curve", CURVES),
                level(),
                ch(),
            ],
            &[
                PathTok { key: "f0", val: PathVal::Num("freq_start") },
                PathTok { key: "f1", val: PathVal::Num("freq_end") },
                PathTok { key: "d", val: PathVal::Num("duration") },
                PathTok { key: "cv", val: PathVal::Choice("curve", CURVE_TOKENS) },
                DB,
                CH,
            ],
            sweep,
        )),
        // a beep train: 1 kHz pips once a second
        Arc::new(sound(
            "beep",
            "genkind.beep",
            AUDIO,
            vec![
                freq("freq", 1000.0),
                level(),
                smooth("period", 0.05, 60.0, 1.0, 0.01, ParamUnit::Sec),
                smooth("length", 1.0, 1000.0, 40.0, 1.0, ParamUnit::Ms),
                ch(),
            ],
            &[
                PathTok { key: "f", val: PathVal::Num("freq") },
                DB,
                PathTok { key: "p", val: PathVal::Num("period") },
                PathTok { key: "len", val: PathVal::Num("length") },
                CH,
            ],
            beep,
        )),
        Arc::new(sound("silence", "genkind.silence", AUDIO, vec![], &[], silence)),
    ]
}
