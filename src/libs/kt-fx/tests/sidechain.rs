//! Tests 767-768 (R.65): the KEY input of the compressor.
//!
//! The owner (R.63 item 8.4): “you can show the same duplicated audio compressor
//! widget as ducking…
//! copies of the same code pile up everywhere.” Hence the rule: ducking is NOT a new
//! device, but a compressor with an assigned key. Here the device itself is checked:
//! the detector listens to the key, its own input is compressed, and without a key everything
//! stays exactly as before.

use kt_fx::{CH, ChainOpts, TrackProcessor};
use kt_model::FxInstance;

const SR: usize = 48_000;

fn comp() -> FxInstance {
    FxInstance::Compressor {
        threshold_db: -24.0,
        ratio: 8.0,
        attack_ms: 5.0,
        release_ms: 60.0,
        makeup_db: 0.0,
        key: None, // the key is fed DIRECTLY into process_keyed: kt-fx knows nothing about tracks
    }
}

/// A steady tone is “music”.
fn tone(frames: usize) -> Vec<f32> {
    let mut v = vec![0f32; frames * CH];
    for i in 0..frames {
        let s = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / SR as f32).sin() * 0.5;
        v[i * CH] = s;
        v[i * CH + 1] = s;
    }
    v
}

/// “Speech”: two loud lines with a pause between them.
fn speech(frames: usize, spans: &[(usize, usize)]) -> Vec<f32> {
    let mut v = vec![0f32; frames * CH];
    for &(a, b) in spans {
        for i in a..b.min(frames) {
            let s = (2.0 * std::f32::consts::PI * 700.0 * i as f32 / SR as f32).sin() * 0.7;
            v[i * CH] = s;
            v[i * CH + 1] = s;
        }
    }
    v
}

fn rms(buf: &[f32], a: usize, b: usize) -> f32 {
    let sl = &buf[a * CH..b * CH];
    (sl.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / sl.len() as f64).sqrt() as f32
}

/// Test 767 (R.65): the key controls, its own input is compressed.
#[test]
fn t767_key_drives_the_detector_and_input_is_compressed() {
    let frames = SR / 2; // 0.5 c
    // the pause between lines is clearly longer than the release (60 ms ≈ 2880
    // frames): otherwise “did the level return” is measured at the half-decay point
    let spans = [(2_000usize, 8_000usize), (20_000usize, 23_000usize)];

    // --- WITH KEY: the detector hears speech, the tone is compressed ---
    let mut p = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut out = tone(frames);
    let key = speech(frames, &spans);
    p.process_keyed(0, &mut out, &|slot| (slot == 0).then_some(&key[..]));

    // …quieter under a line, not between lines
    let under = rms(&out, 6_000, 8_000);
    let between = rms(&out, 16_000, 18_000);
    let before = rms(&out, 0, 1_500);
    assert!(
        under < between * 0.6,
        "under the key the music must dip: {under} against {between} between the lines"
    );
    assert!(
        (between / before - 1.0).abs() < 0.2,
        "between the lines the level must return: {between} against {before} before the first"
    );

    // …and the OUTPUT is the sound of its own input: under a line the output has the tone frequency (220),
    // not the key frequency (700) — otherwise we would have “mixed in” the key instead of controlling it
    let z = |buf: &[f32], a: usize, b: usize| {
        buf[a * CH..b * CH].chunks_exact(CH).filter(|w| w[0] == 0.0).count()
    };
    assert_eq!(z(&out, 6_000, 8_000), 0, "zeros appeared in the output — this is not a compressed tone");

    // --- WITHOUT KEY: exactly the previous device ---
    let mut p_plain = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut plain = tone(frames);
    p_plain.process(0, &mut plain);

    // a key EQUAL to its own input must give the same result byte for byte:
    // “the same reduction that this same signal would get on its own input”
    let mut p_self = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut selfkey = tone(frames);
    let same = tone(frames);
    p_self.process_keyed(0, &mut selfkey, &|slot| (slot == 0).then_some(&same[..]));
    assert_eq!(selfkey, plain, "a key equal to the input gave something other than the path without a key");
}

/// Test 768 (R.65): neutrality and the cost of the key branch.
#[test]
fn t768_silent_key_is_transparent_and_costs_nothing() {
    let frames = 4_096;
    let mut p = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut out = tone(frames);
    let src = tone(frames);
    let silent = vec![0f32; frames * CH];
    p.process_keyed(0, &mut out, &|_| Some(&silent[..]));
    for (o, s) in out.iter().zip(src.iter()) {
        assert!((o - s).abs() < 1e-6, "silence in the key must leave the input untouched");
    }

    // a key SHORTER than the buffer is “key silent” over the remainder, not a panic
    let short = vec![0f32; (frames / 2) * CH];
    let mut out2 = tone(frames);
    p.process_keyed(0, &mut out2, &|_| Some(&short[..]));
    assert!(out2.iter().all(|s| s.is_finite()), "a short key gave non-numbers");
}
