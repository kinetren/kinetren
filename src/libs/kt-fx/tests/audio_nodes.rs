//! Тесты 319+ (Р.47): аудио-примитивы и узловая цепочка kt-fx.

use kt_fx::{CH, Freeverb};

/// Тест 319 (п.324): reset() реверба обязан зачищать И predelay-кольцо.
/// До Р.47 занулялись только comb/allpass: при predelay_ms > 0 до 200 мс
/// СТАРОГО материала протекали в хвост после seek — рендер с нуля и рендер
/// после перемотки расходились (инвариант №11). Red-proof: на коде до
/// фикса максимум выхода на нулевом входе после reset() — десятки dBFS.
#[test]
fn t319_reverb_reset_clears_predelay() {
    let mut rv = Freeverb::new(1.0, 100.0, 0.2, 0.7, 1.0, 120.0, 1.0);
    // прогреть кольцо пред-задержки громким материалом
    let mut buf = vec![0f32; 4096 * CH];
    for (i, s) in buf.iter_mut().enumerate() {
        *s = if i % 97 == 0 { 0.9 } else { 0.3 };
    }
    rv.process(&mut buf);
    rv.reset();
    // после полного сброса нулевой вход обязан давать строго нулевой выход
    let mut silent = vec![0f32; 48_000 * CH];
    rv.process(&mut silent);
    let peak = silent.iter().fold(0f32, |m, s| m.max(s.abs()));
    assert_eq!(peak, 0.0, "predelay leaked stale material after reset: peak {peak}");
}

/// Юниты примитивов Р.47 (п.324): АЧХ биквада, дробная задержка, интеграл
/// параметра, огибающая. Пороги грубые — это проверка «формула та», а не
/// калибровка.
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
        // последняя запись 31: задержка 1 => 31, задержка 2 => 30
        assert_eq!(d.read_frac(1.0), 31.0);
        assert_eq!(d.read_frac(2.0), 30.0);
        // дробная: между 30 и 31
        assert!((d.read_frac(1.5) - 30.5).abs() < 1e-6);
    }

    #[test]
    fn param_integral_trapezoid() {
        // плоская база без точек: интеграл = base*t
        assert_eq!(kt_model::param_integral(5.0, &[], 2.0), 10.0);
        // рампа 0->10 на [0,1]: интеграл к 1 = 5; к 0.5 = 1.25
        let pts = vec![(0.0, 0.0), (1.0, 10.0)];
        assert!((kt_model::param_integral(0.0, &pts, 1.0) - 5.0).abs() < 1e-9);
        assert!((kt_model::param_integral(0.0, &pts, 0.5) - 1.25).abs() < 1e-9);
        // за последней точкой — константа
        assert!((kt_model::param_integral(0.0, &pts, 2.0) - 15.0).abs() < 1e-9);
        // значения НЕ клампятся (в отличие от source_progress)
        let fast = vec![(0.0, 100.0), (1.0, 100.0)];
        assert!((kt_model::param_integral(1.0, &fast, 1.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn env_follower_times() {
        let mut e = EnvFollower::new(10.0, 100.0);
        // ступенька вверх: к 63% примерно за attack
        let att_frames = (0.010 * 48_000.0) as usize;
        let mut v = 0.0;
        for _ in 0..att_frames {
            v = e.tick(1.0);
        }
        assert!(v > 0.55 && v < 0.72, "attack 63% point: {v}");
        // отпустили: релиз медленнее
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
