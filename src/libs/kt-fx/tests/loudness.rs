//! Tests 403-405 (R.49 B4): loudness per BS.1770 / EBU R 128.

use kt_fx::audio::loudness::{LoudnessMeter, measure};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

/// A sine of a given amplitude and length.
fn sine(hz: f64, amp: f32, secs: f64) -> Vec<f32> {
    let n = (AUDIO_SAMPLE_RATE as f64 * secs) as usize;
    let mut v = Vec::with_capacity(n * AUDIO_CHANNELS);
    for i in 0..n {
        let s = (2.0 * std::f64::consts::PI * hz * i as f64 / AUDIO_SAMPLE_RATE as f64).sin();
        let x = (s as f32) * amp;
        for _ in 0..AUDIO_CHANNELS {
            v.push(x);
        }
    }
    v
}

/// Test 403 (R.49 B4): CALIBRATION of the scale.
///
/// The BS.1770 formula: `L = −0.691 + 10·log10(Σ G_i · z_i)`, where `z_i` is the mean
/// square of the K-filtered channel. The offset −0.691 was chosen not at random: it
/// exactly cancels the gain of the K-filter at 1 kHz for a stereo pair, and a full-scale
/// 1 kHz sine fed into both channels reads as 0.0 LUFS. Hence the
/// check: a −20 dBFS tone must give −20.0 LUFS.
///
/// The EBU tolerance is 0.1 LU, and this is the reason the K-filter is computed in f64:
/// the coefficient `-1.99004745483398` is truncated in f32, and the biquad is RECURSIVE —
/// the pole error accumulates over the whole file.
#[test]
fn t403_scale_is_calibrated() {
    // full scale into both channels — zero by the definition of the calibration
    let full = measure(&sine(1000.0, 1.0, 20.0)).lufs.expect("a tone cannot be below the gate");
    assert!(
        full.abs() <= 0.1,
        "a full-scale 1 kHz sine in the stereo pair must give 0.0 LUFS ±0.1, got {full:.3}"
    );

    // −20 dBFS: amplitude 0.1; 20 seconds — enough blocks for the gates
    let amp = 10f32.powf(-20.0 / 20.0);
    let r = measure(&sine(1000.0, amp, 20.0));
    let lufs = r.lufs.expect("a tone cannot be below the absolute gate");
    assert!(
        (lufs + 20.0).abs() <= 0.1,
        "a −20 dBFS tone must give −20.0 LUFS ±0.1, got {lufs:.3}"
    );
    // the same tone 10 dB quieter — exactly 10 LU quieter
    let quiet = measure(&sine(1000.0, amp * 10f32.powf(-10.0 / 20.0), 20.0));
    let q = quiet.lufs.unwrap();
    assert!(((lufs - q) - 10.0).abs() <= 0.1, "the scale is not linear: {lufs:.3} against {q:.3}");
    // a flat tone — this is a ZERO loudness range
    assert!(r.lra <= 1.0, "a steady tone must have LRA near zero, got {:.2}", r.lra);

    // silence is not “−70” but “no number” — lying about a loudness that does not
    // exist is worse than honestly staying silent
    let silence = vec![0.0f32; AUDIO_SAMPLE_RATE as usize * AUDIO_CHANNELS];
    assert_eq!(measure(&silence).lufs, None);
}

/// The reason loudness is computed in f64 is the SUMS, not the coefficients.
/// The test guards the reason itself: it shows that direct accumulation of squares
/// in f32 drifts past the EBU tolerance (0.1 LU), and so moving the accumulators
/// to single precision “for speed” is a regression, not an
/// optimization.
#[test]
fn t403b_f32_accumulator_drifts_past_the_ebu_tolerance() {
    let sr = AUDIO_SAMPLE_RATE as usize;
    let n = sr * 600; // ten minutes
    let amp = 0.1f64;
    let (mut a32, mut a64) = (0.0f32, 0.0f64);
    for i in 0..n {
        let x = (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr as f64).sin() * amp;
        a32 += (x * x) as f32;
        a64 += x * x;
    }
    let m32 = a32 as f64 / n as f64;
    let m64 = a64 / n as f64;
    let drift = 10.0 * (m32 / m64).log10();
    assert!(
        drift.abs() > 0.1,
        "f32 accumulation fit within tolerance ({drift:.3} LU) — so the module comment is untrue \
         and the reason for f64 must be named differently"
    );
    // and the meter itself, computing in f64, is accurate on the same material
    let r = measure(&sine(1000.0, amp as f32, 60.0)).lufs.unwrap();
    assert!((r + 20.0).abs() <= 0.1, "the f64 meter drifted: {r:.3} LUFS");
}

/// Test 404 (R.49 B4): the INTER-SAMPLE overshoot is visible.
///
/// Between the samples the reconstructed wave goes HIGHER than at them: a sine at
/// a quarter of the sampling frequency, phase-shifted by 45°, has samples
/// exactly at ±0.707·A, while the true peak is A. The peak by the samples would say
/// “−3 dBFS, there is headroom”, and after the codec there would be clipping.
#[test]
fn t404_true_peak_sees_the_intersample_overshoot() {
    let n = AUDIO_SAMPLE_RATE as usize * 2;
    let mut v = Vec::with_capacity(n * AUDIO_CHANNELS);
    let mut sample_peak = 0.0f32;
    for i in 0..n {
        // 12 kHz at 48 kHz = a quarter of the frequency; a 45° phase puts the samples
        // at the points ±1/√2, and the wave's maximum is between them
        let phase = std::f64::consts::PI * 0.25 + std::f64::consts::PI * 0.5 * i as f64;
        let x = phase.sin() as f32;
        sample_peak = sample_peak.max(x.abs());
        for _ in 0..AUDIO_CHANNELS {
            v.push(x);
        }
    }
    let db = |a: f32| 20.0 * (a as f64).log10();
    let r = measure(&v);
    assert!(
        (db(sample_peak) + 3.0).abs() < 0.2,
        "scene: the sample peak must be about −3 dBFS, got {:.2}",
        db(sample_peak)
    );
    assert!(
        r.dbtp > db(sample_peak) + 1.5,
        "the true peak did not see an inter-sample overshoot: {:.2} dBTP against {:.2} dBFS by samples",
        r.dbtp,
        db(sample_peak)
    );
    assert!(r.dbtp <= 0.5, "the true peak is overstated: {:.2} dBTP with a waveform maximum of 0 dBFS", r.dbtp);
}

/// Test 405 (R.49 B4): the SLICING does not affect the result.
///
/// The meter is fed what it is fed: export chunks, pyramid bursts,
/// a whole buffer. If the block grid or the filters lived in the call rather than in the
/// accumulator, the same audio would give different numbers depending on
/// who fed it.
#[test]
fn t405_chunking_does_not_change_the_result() {
    // material with SILENCE and loud pieces: the gates must work
    let mut data = Vec::new();
    let amp = 10f32.powf(-20.0 / 20.0);
    for k in 0..6 {
        if k % 2 == 0 {
            data.extend(sine(1000.0, amp, 2.0));
        } else {
            data.extend(vec![0.0f32; AUDIO_SAMPLE_RATE as usize * AUDIO_CHANNELS]);
        }
    }
    let whole = measure(&data);

    for chunk_frames in [1usize, 1024, 4096, 7919] {
        let mut m = LoudnessMeter::new();
        for c in data.chunks(chunk_frames * AUDIO_CHANNELS) {
            m.feed(c);
        }
        let got = m.finish();
        let a = whole.lufs.unwrap();
        let b = got.lufs.unwrap();
        assert!(
            (a - b).abs() < 1e-9,
            "chunking by {chunk_frames} frames changed the loudness: {a} against {b}"
        );
        assert!((whole.dbtp - got.dbtp).abs() < 1e-9, "chunking changed the true peak");
        assert!((whole.lra - got.lra).abs() < 1e-9, "chunking changed the range");
    }

    // and the gates work: the pauses did not pull the result down to −70
    // half of the material is silence. Without gates the mean energy would be half
    // as much, that is about −23 LUFS; with the gates the result must stay at the
    // loudness of the phrases themselves. A difference of about half a LU comes from blocks AT THE
    // JUNCTION of tone and silence — they are above the relative gate and legitimately enter the count.
    let l = whole.lufs.unwrap();
    assert!(
        l > -21.5,
        "pauses dragged the loudness to {l:.2} LUFS — the gates did not work (without them it would be \
         about −23, with them about −20 is expected)"
    );
    assert!(l < -19.5, "loudness is overstated: {l:.2} LUFS with phrases at −20");
}
