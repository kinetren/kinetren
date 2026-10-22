//! Tests 319+ (R.47): audio primitives and the node chain of kt-fx.

use kt_fx::{CH, Freeverb};

/// Test 319 (item 324): the reverb reset() must clear BOTH the predelay ring.
/// Before R.47 only comb/allpass were zeroed: with predelay_ms > 0, up to 200 ms of
/// OLD material leaked into the tail after a seek — a render from scratch and a render
/// after a seek diverged (invariant No. 11). Red-proof: on the code before the
/// fix the maximum output on a zero input after reset() — tens of dBFS.
#[test]
fn t319_reverb_reset_clears_predelay() {
    let mut rv = Freeverb::new(1.0, 100.0, 0.2, 0.7, 1.0, 120.0, 1.0);
    // warm the pre-delay ring with loud material
    let mut buf = vec![0f32; 4096 * CH];
    for (i, s) in buf.iter_mut().enumerate() {
        *s = if i % 97 == 0 { 0.9 } else { 0.3 };
    }
    rv.process(&mut buf);
    rv.reset();
    // after a full reset a zero input must give strictly zero output
    let mut silent = vec![0f32; 48_000 * CH];
    rv.process(&mut silent);
    let peak = silent.iter().fold(0f32, |m, s| m.max(s.abs()));
    assert_eq!(peak, 0.0, "predelay leaked stale material after reset: peak {peak}");
}

/// Unit tests of the R.47 primitives (item 324): biquad response, fractional delay, integral of
/// a parameter, envelope. The thresholds are coarse — this is a check that “the formula is right”,
/// not a calibration.
mod primitives {
    use kt_fx::audio::primitives::{Biquad, BiquadKind, DelayLine, EnvFollower, softclip};

    fn rms_at(filter: &mut Biquad, hz: f32) -> f32 {
        let sr = 48_000.0f32;
        let n = 48_000usize;
        let mut acc = 0f64;
        for i in 0..n {
            let x = (2.0 * std::f32::consts::PI * hz * i as f32 / sr).sin();
            let y = filter.process(x);
            if i >= n / 2 {
                acc += (y as f64) * (y as f64);
            }
        }
        ((acc / (n / 2) as f64).sqrt() * std::f64::consts::SQRT_2) as f32
    }

    #[test]
    fn biquad_lowpass_shape() {
        let mut f = Biquad::default();
        f.tune(&BiquadKind::Lowpass, 800.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
        let pass = rms_at(&mut f, 200.0);
        f.reset();
        let cut = rms_at(&mut f, 8_000.0);
        f.reset();
        let edge = rms_at(&mut f, 800.0);
        assert!(pass > 0.95 && pass < 1.05, "passband {pass}");
        assert!(cut < 0.02, "8 kHz must be ~40 dB down: {cut}");
        assert!((edge - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.05, "-3 dB at cutoff: {edge}");
    }

    #[test]
    fn delay_line_fractional_read() {
        let mut d = DelayLine::new(64);
        for i in 0..32 {
            d.write(i as f32);
        }
        // last entry 31: delay 1 => 31, delay 2 => 30
        assert_eq!(d.read_frac(1.0), 31.0);
        assert_eq!(d.read_frac(2.0), 30.0);
        // fractional: between 30 and 31
        assert!((d.read_frac(1.5) - 30.5).abs() < 1e-6);
    }

    #[test]
    fn param_integral_trapezoid() {
        // flat base without points: integral = base*t
        assert_eq!(kt_model::param_integral(5.0, &[], 2.0), 10.0);
        // ramp 0->10 on [0,1]: integral at 1 = 5; at 0.5 = 1.25
        let pts = vec![(0.0, 0.0), (1.0, 10.0)];
        assert!((kt_model::param_integral(0.0, &pts, 1.0) - 5.0).abs() < 1e-9);
        assert!((kt_model::param_integral(0.0, &pts, 0.5) - 1.25).abs() < 1e-9);
        // after the last point — a constant
        assert!((kt_model::param_integral(0.0, &pts, 2.0) - 15.0).abs() < 1e-9);
        // values are NOT clamped (unlike source_progress)
        let fast = vec![(0.0, 100.0), (1.0, 100.0)];
        assert!((kt_model::param_integral(1.0, &fast, 1.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn env_follower_times() {
        let mut e = EnvFollower::new(10.0, 100.0);
        // a step up: reaches 63% in about attack
        let att_frames = (0.010 * 48_000.0) as usize;
        let mut v = 0.0;
        for _ in 0..att_frames {
            v = e.tick(1.0);
        }
        assert!(v > 0.55 && v < 0.72, "attack 63% point: {v}");
        // released: the release is slower
        let mut w = v;
        for _ in 0..att_frames {
            w = e.tick(0.0);
        }
        assert!(w > v * 0.7, "release must be slower than attack: {w} vs {v}");
    }

    #[test]
    fn softclip_is_bounded_and_odd() {
        assert_eq!(softclip(0.0), 0.0);
        assert!((softclip(1.0) - 0.5).abs() < 1e-7);
        assert_eq!(softclip(-2.0), -softclip(2.0));
        assert!(softclip(1e6) < 1.0);
    }
}

/// Test 348 (R.48, item 347): the equalizer's frequency response is computed from THE SAME
/// coefficients that sound.
///
/// The function is needed by the graphics in the FX window, and the cost of an error in it is a picture
/// that lies about the sound. Therefore the semantic points are checked: zeros
/// do nothing, the peak equals its gain AT its own frequency and
/// decays at the edges, shelves raise their own edge, Q narrows.
#[test]
fn t348_eq_mag_db_matches_band_semantics() {
    // v: low_gain, low_freq, mid_gain, mid_freq, mid_q, high_gain, high_freq
    let flat = [0.0, 200.0, 0.0, 1000.0, 1.0, 0.0, 4000.0];
    for hz in [20.0f32, 60.0, 200.0, 1000.0, 5000.0, 12000.0, 20000.0] {
        let db = kt_fx::eq_mag_db(&flat, hz);
        assert!(db.abs() < 0.01, "zero gains must be an identity: {hz} Hz → {db} dB");
    }

    // a +6 dB peak at 1 kHz: at its frequency exactly +6, at the edges ~0
    let peak = [0.0, 200.0, 6.0, 1000.0, 1.0, 0.0, 4000.0];
    let at_f0 = kt_fx::eq_mag_db(&peak, 1000.0);
    println!("t348 peak at f0: {at_f0:.3} dB");
    assert!((at_f0 - 6.0).abs() < 0.05, "at its own frequency the peak equals the gain: {at_f0}");
    assert!(kt_fx::eq_mag_db(&peak, 40.0).abs() < 0.3, "far below the peak is not audible");
    assert!(kt_fx::eq_mag_db(&peak, 16000.0).abs() < 0.3, "and far above too");

    // Q narrows: at Q=8 the neighbour in frequency is affected less
    let wide = [0.0, 200.0, 6.0, 1000.0, 0.4, 0.0, 4000.0];
    let narrow = [0.0, 200.0, 6.0, 1000.0, 8.0, 0.0, 4000.0];
    let (w, n) = (kt_fx::eq_mag_db(&wide, 2000.0), kt_fx::eq_mag_db(&narrow, 2000.0));
    println!("t348 at 2 kHz: wide {w:.2} dB, narrow {n:.2} dB");
    assert!(n < w - 1.0, "a high Q must narrow the band: {n} against {w}");

    // shelves: the low one raises the bottom and does not touch the top, the high one the other way round
    let low = [-12.0, 200.0, 0.0, 1000.0, 1.0, 0.0, 4000.0];
    assert!(kt_fx::eq_mag_db(&low, 20.0) < -10.0, "a low shelf must cut the bottom");
    assert!(kt_fx::eq_mag_db(&low, 16000.0).abs() < 0.2, "and must not touch the top");
    let high = [0.0, 200.0, 0.0, 1000.0, 1.0, 6.0, 4000.0];
    assert!(kt_fx::eq_mag_db(&high, 20000.0) > 4.0, "a high shelf must lift the top");
    assert!(kt_fx::eq_mag_db(&high, 40.0).abs() < 0.2, "and must not touch the bottom");

    // the cascade adds up: two bands together give the sum of their contributions
    let both = [-12.0, 200.0, 6.0, 1000.0, 1.0, 0.0, 4000.0];
    let sum = kt_fx::eq_mag_db(&low, 1000.0) + kt_fx::eq_mag_db(&peak, 1000.0);
    assert!(
        (kt_fx::eq_mag_db(&both, 1000.0) - sum).abs() < 1e-9,
        "a cascade is the sum in dB of its bands"
    );
}
