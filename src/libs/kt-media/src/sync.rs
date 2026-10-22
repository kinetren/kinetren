//! Offset of two recordings by sound (R.49, item 374).
//!
//! The editor's task: the camera recorded its own sound, the lav mic its own, and the clapper
//! was missing or not visible. We need the offset at which the tracks line up.
//!
//! **Why by the PEAK PYRAMID, not by samples.** The pyramid is already built
//! (the index DB builds it for waveforms), lies in memory and is a hundred times shorter
//! than the signal. Correlation over the samples of hours of material is hours of computing;
//! over the top pyramid level it is milliseconds, and descending the levels brings
//! the precision down to the frame.
//!
//! **Why by the POSITIVE PART OF THE FIRST DIFFERENCE of the envelope,
//! not the envelope itself.** Two recordings of one event were made with
//! different microphones: the lav has a close voice without the room, the camera has
//! the room, echo and noise. The shapes of their envelopes are not alike.
//! But the TRANSIENTS — the moments when sound sharply starts — are common
//! to them: these are the same events in the room. The positive part of the difference
//! keeps exactly them and discards the difference in frequency response, level and reverberation.
//!
//! **Sign.** `offset(a, b)` — how much later B starts relative to A.
//! A positive value means “B lags”: to align, B must be shifted BACK.
//! Swapping the arguments must give exactly the negative of the same offset —
//! a classic correlation mistake, guarded by a test.

use kt_time::Tick;

/// Result precision: the 90 kHz tick divides it evenly, and this is less than
/// half a frame at any project fps.
pub const SYNC_GRANULARITY_TICKS: i64 = 480;

/// Confidence threshold. Below it NO ANSWER IS RETURNED: an offset “by eye” is worse
/// than a refusal — the editor would move a track, hear a flanger, and look for the
/// cause anywhere but in the automation. 0.30 is an empirical edge beyond which, for
/// recordings with different microphones, the correlation peak stops differing from the
/// background (verified on noise pairs, t415).
pub const SYNC_MIN_CONFIDENCE: f64 = 0.30;

/// Result: the offset of B relative to A and confidence 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncOffset {
    pub ticks: Tick,
    pub confidence: f64,
}

/// The level envelope from a pyramid level: the half-range of the block.
///
/// The RANGE (max − min) is taken, not the maximum: it does not depend on the DC component,
/// which is noticeable in cheap microphones.
fn envelope(level: &[(i16, i16)]) -> Vec<i32> {
    level.iter().map(|&(lo, hi)| hi as i32 - lo as i32).collect()
}

/// The positive part of the first difference — “where the sound STARTED”.
///
/// The first block is compared WITH ITSELF, not with zero: otherwise
/// any recording's zero block produces a huge false “onset”, identical in all pairs,
/// and the correlation of two UNRELATED noises at zero shift shoots up
/// to almost one (found t415).
fn onsets(env: &[i32]) -> Vec<f64> {
    let mut v = Vec::with_capacity(env.len());
    let mut prev = env.first().copied().unwrap_or(0);
    for &e in env {
        v.push((e - prev).max(0) as f64);
        prev = e;
    }
    v
}

/// Normalization to zero mean and unit norm: after it the dot product is the
/// correlation coefficient.
fn normalize(mut v: Vec<f64>) -> Vec<f64> {
    if v.is_empty() {
        return v;
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    for x in v.iter_mut() {
        *x -= mean;
    }
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-12 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    v
}

/// Correlation at a shift of `lag` blocks, where a POSITIVE lag means “B
/// starts LATER than A by lag blocks”: `a[i]` is compared with `b[i + lag]`.
///
/// The sign here is no trifle. With `b[j] = a[j − D]` (B lags by D) the maximum
/// must fall on `lag = D`; if confused, “shift by +0.4 s” turns into
/// “shift by −0.4 s”, and the tracks drift twice as far apart as they were
/// (guard — t414).
///
/// A short overlap cannot win: when normalizing over the WHOLE length, the sum
/// over a piece is not bounded by anything, and a dozen blocks at the edge would give
/// a “confident” answer about nothing.
fn corr_at(a: &[f64], b: &[f64], lag: i64) -> f64 {
    let n = a.len() as i64;
    let m = b.len() as i64;
    let from = (-lag).max(0);
    let to = n.min(m - lag);
    if to <= from {
        return 0.0;
    }
    let overlap = to - from;
    if overlap * 4 < n.min(m) {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in from..to {
        sum += a[i as usize] * b[(i + lag) as usize];
    }
    sum
}

/// Best shift in blocks and confidence.
///
/// The tie-break is DETERMINISTIC: with equal values the SMALLER in absolute value shift is taken,
/// and at equal absolute value — the negative one. Without an explicit rule the result would
/// depend on the traversal order, and “recalculated the same thing” would sometimes give
/// a different answer.
fn best_lag(a: &[f64], b: &[f64], lo: i64, hi: i64) -> (i64, f64) {
    let mut best = (0i64, f64::NEG_INFINITY);
    for lag in lo..=hi {
        let c = corr_at(a, b, lag);
        let better = c > best.1 + 1e-12
            || ((c - best.1).abs() <= 1e-12
                && (lag.abs() < best.0.abs() || (lag.abs() == best.0.abs() && lag < best.0)));
        if better {
            best = (lag, c);
        }
    }
    best
}

/// Offset of B relative to A from the peak pyramids of one channel.
///
/// `levels_a`/`levels_b` — pyramid levels (level 0 is the most detailed),
/// `frames_per_block0` — how many audio frames are in a level-0 block.
pub fn offset_from_pyramids(
    levels_a: &[Vec<(i16, i16)>],
    levels_b: &[Vec<(i16, i16)>],
    frames_per_block0: usize,
) -> Option<SyncOffset> {
    let depth = levels_a.len().min(levels_b.len());
    if depth == 0 || frames_per_block0 == 0 {
        return None;
    }
    // Descent through the levels: coarse and fast at the top, precise and narrow at the bottom.
    //
    // The starting level is chosen BY LENGTH, not “the topmost”: at the apex of the pyramid there are
    // one or two blocks, a full search there is pointless, and if one starts from it, not a single
    // full search would happen at all — only narrow neighborhoods around the not-found zero would follow
    // (found t414).
    const MIN_BLOCKS: usize = 16;
    let start = (0..depth)
        .rev()
        .find(|&l| levels_a[l].len() >= MIN_BLOCKS && levels_b[l].len() >= MIN_BLOCKS)
        .unwrap_or(0);
    let mut coarse_lag = 0i64; // in blocks of the CURRENT level
    let mut confidence;
    let mut level = start;
    loop {
        let a = normalize(onsets(&envelope(&levels_a[level])));
        let b = normalize(onsets(&envelope(&levels_b[level])));
        let (lo, hi) = if level == start {
            // first pass — the whole admissible range
            (-(a.len() as i64 - 1), b.len() as i64 - 1)
        } else {
            // after that — only the neighborhood of the found one: ±2 blocks of the parent
            (coarse_lag - 4, coarse_lag + 4)
        };
        let (lag, c) = best_lag(&a, &b, lo, hi);
        coarse_lag = lag;
        confidence = c;
        if level == 0 {
            break;
        }
        level -= 1;
        coarse_lag *= 2; // a block of the next level is half as long
    }
    if confidence.partial_cmp(&SYNC_MIN_CONFIDENCE) != Some(std::cmp::Ordering::Greater) {
        return None; // NaN belongs here too: “don't know” is a refusal, not zero
    }
    let frames = coarse_lag * frames_per_block0 as i64;
    let ticks = Tick::from_audio_frames(frames);
    // rounding to the grid: promising precision above a pyramid block is dishonest
    let q =
        (ticks.0 as f64 / SYNC_GRANULARITY_TICKS as f64).round() as i64 * SYNC_GRANULARITY_TICKS;
    Some(SyncOffset { ticks: Tick(q), confidence })
}
