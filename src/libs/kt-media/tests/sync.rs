//! Tests 413-415 (R.49 B8): offset of two recordings by sound.

use kt_media::PEAK_BLOCK_FRAMES;
use kt_media::sync::{SYNC_GRANULARITY_TICKS, offset_from_pyramids};
use kt_time::AUDIO_SAMPLE_RATE;

/// A deterministic “noise” without external dependencies.
fn rng(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32 / (1u64 << 31) as f32) - 1.0
}

/// Scene: strikes at given seconds, with a given “color” and level.
/// `lowpass` roughly imitates another microphone (a camera deep in the room),
/// `noise` — the room's noise.
fn take(hits: &[f64], secs: f64, lowpass: bool, level: f32, noise: f32, seed0: u64) -> Vec<f32> {
    let sr = AUDIO_SAMPLE_RATE as usize;
    let n = (sr as f64 * secs) as usize;
    let mut v = vec![0.0f32; n];
    let mut seed = seed0;
    for &h in hits {
        let at = (h * sr as f64) as usize;
        // a strike: a sharp attack, exponential decay of 150 ms
        for i in 0..(sr / 6) {
            if at + i >= n {
                break;
            }
            let env = (-(i as f32) / (sr as f32 * 0.03)).exp();
            let tone = (2.0 * std::f32::consts::PI * 180.0 * i as f32 / sr as f32).sin();
            v[at + i] += env * tone * level;
        }
    }
    if lowpass {
        // a one-pole LP — “another microphone”: the envelope shape changes
        let mut z = 0.0f32;
        for x in v.iter_mut() {
            z += 0.08 * (*x - z);
            *x = z;
        }
    }
    if noise > 0.0 {
        for x in v.iter_mut() {
            *x += rng(&mut seed) * noise;
        }
    }
    v
}

/// A pyramid from a mono signal: exactly the same arithmetic as
/// `PeakPyramid::build` (a block — min and max, the level above — pairwise
/// merge). It is built here, not by the source, so that the test does not depend on the
/// set of synthetic patterns.
fn pyramid(mono: &[f32]) -> Vec<Vec<(i16, i16)>> {
    let mut level0: Vec<(i16, i16)> = Vec::new();
    for chunk in mono.chunks(PEAK_BLOCK_FRAMES) {
        let mut lo = i16::MAX;
        let mut hi = i16::MIN;
        for &x in chunk {
            let v = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        level0.push((lo, hi));
    }
    let mut levels = vec![level0];
    while levels.last().unwrap().len() > 1 {
        let prev = levels.last().unwrap();
        let next: Vec<(i16, i16)> = prev
            .chunks(2)
            .map(|p| (p.iter().map(|x| x.0).min().unwrap(), p.iter().map(|x| x.1).max().unwrap()))
            .collect();
        levels.push(next);
    }
    levels
}

/// Test 413 (R.49 B8): the offset is found even when the recordings sound differently.
/// The microphones differ: one has a sagging frequency response, its level is half, and
/// room noise is added. They share only the MOMENTS of events — that is what is computed on.
#[test]
fn t413_finds_the_offset_across_different_microphones() {
    let hits = [0.7, 1.9, 2.4, 3.6, 4.1, 5.5, 6.2];
    let shift = 0.4; // B lags by 400 ms
    let a = take(&hits, 8.0, false, 0.8, 0.0, 1);
    let b_hits: Vec<f64> = hits.iter().map(|h| h + shift).collect();
    let b = take(&b_hits, 8.0, true, 0.35, 0.02, 7);

    let r = offset_from_pyramids(&pyramid(&a), &pyramid(&b), PEAK_BLOCK_FRAMES)
        .expect("the shift must be found");
    let want = (shift * 90_000.0) as i64;
    let block_ticks = (PEAK_BLOCK_FRAMES as i64 * 90_000) / AUDIO_SAMPLE_RATE as i64;
    assert!(
        (r.ticks.0 - want).abs() <= block_ticks,
        "shift {} ticks against expected {want} (tolerance — a pyramid block, {block_ticks})",
        r.ticks.0
    );
    assert!(r.confidence > 0.3, "confidence {} too low", r.confidence);
    assert_eq!(r.ticks.0 % SYNC_GRANULARITY_TICKS, 0, "result did not land on the precision grid");
}

/// Test 414 (R.49 B8): SWAPPING the arguments gives exactly the negative of the same offset.
///
/// A classic correlation mistake: the lag sign is lost, and “shift the second file
/// by +0.4 s” turns into “shift by −0.4 s”. In that case the editor
/// drives the tracks twice as far from each other as they were.
#[test]
fn t414_swapping_arguments_flips_the_sign() {
    let hits = [0.5, 1.4, 2.8, 3.3, 4.9, 5.6];
    let a = take(&hits, 7.0, false, 0.8, 0.0, 3);
    let b_hits: Vec<f64> = hits.iter().map(|h| h + 0.3).collect();
    let b = take(&b_hits, 7.0, false, 0.8, 0.0, 3);
    let (pa, pb) = (pyramid(&a), pyramid(&b));

    let ab = offset_from_pyramids(&pa, &pb, PEAK_BLOCK_FRAMES).expect("forward order");
    let ba = offset_from_pyramids(&pb, &pa, PEAK_BLOCK_FRAMES).expect("reverse order");
    assert!(ab.ticks.0 > 0, "B lags — the shift must be positive, got {}", ab.ticks.0);
    assert_eq!(
        ab.ticks.0, -ba.ticks.0,
        "swapping the arguments did not give the negated same shift: {} and {}",
        ab.ticks.0, ba.ticks.0
    );
    // and a repeat gives the same: the tie-break is deterministic
    let again = offset_from_pyramids(&pa, &pb, PEAK_BLOCK_FRAMES).unwrap();
    assert_eq!(ab.ticks, again.ticks);
    assert_eq!(ab.confidence, again.confidence);
}

/// Test 415 (R.49 B8): UNRELATED recordings give a refusal, not zero.
///
/// The answer “offset 0” is indistinguishable from “they already line up”, and the editor will believe it.
/// An offset by eye is worse than a refusal: it will move a track, hear a flanger and
/// look for the cause anywhere but in the automation.
#[test]
fn t415_unrelated_takes_are_refused_not_guessed() {
    let a = take(&[0.6, 1.7, 2.9, 4.4], 6.0, false, 0.8, 0.02, 11);
    // different events at different moments — nothing in common
    let b = take(&[0.2, 2.2, 3.1, 5.2], 6.0, false, 0.8, 0.02, 99);
    let r = offset_from_pyramids(&pyramid(&a), &pyramid(&b), PEAK_BLOCK_FRAMES);
    assert!(
        r.is_none() || r.unwrap().confidence <= 0.6,
        "a confident shift {r:?} was found for unrelated records — the program is making it up"
    );

    // pure noise against pure noise — refusal
    let mut s1 = 5u64;
    let mut s2 = 500u64;
    let n = AUDIO_SAMPLE_RATE as usize * 4;
    let n1: Vec<f32> = (0..n).map(|_| rng(&mut s1) * 0.3).collect();
    let n2: Vec<f32> = (0..n).map(|_| rng(&mut s2) * 0.3).collect();
    let r = offset_from_pyramids(&pyramid(&n1), &pyramid(&n2), PEAK_BLOCK_FRAMES);
    assert!(r.is_none(), "a shift was found between two different noises: {r:?}");
}
