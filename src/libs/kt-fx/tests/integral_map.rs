//! Позиция источника на выходе == интеграл профиля скорости (Р.37).
//!
//! До этого раунда файл только ПЕЧАТАЛ расхождение и не утверждал ничего —
//! то есть «DSP следует карте» неявно сторожил только GOLDEN_AUDIO. Теперь
//! это проверяется прямо: карта в домене события (`S(tau) = ∫rate`), и
//! процессор обязан идти по ней на каждом чанке.
use kt_fx::{CH, ClipProcessor, source_progress};
use kt_model::FxInstance;


/// Профиль (доля ВЫХОДНОГО времени, rate) → кривые клипа и профили kt-fx.
fn profiles(points: &[(f64, f64)], len_frames: i64) -> kt_fx::AutoProfiles {
    let curves = vec![vec![kt_model::ParamCurve {
        param: "rate".into(),
        ease: false,
        keys: points
            .iter()
            .map(|(s, r)| kt_model::AutoKey {
                t: kt_time::Tick::from_audio_frames((s * len_frames as f64).round() as i64),
                v: *r,
            })
            .collect(),
    }]];
    kt_fx::AutoProfiles::from_curves(&curves, 0, len_frames)
}

#[test]
fn clip_processor_follows_integral_map() {
    let automation = vec![
        (0.2, 1.0),
        (0.3, 0.35),
        (0.7, 0.6),
        (0.8, 1.0),
    ];
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        automation: vec![],
    }];
    let len = 20 * 48_000i64;
    let auto = profiles(&automation, len);
    let mut p = ClipProcessor::new(&fx, &auto, 0, len);
    // источник = ramp (значение = номер кадра * 1e-6)
    let mut pull = |_k: kt_fx::PullKind, f0: i64, buf: &mut [f32]| {
        for (i, s) in buf.iter_mut().enumerate() {
            *s = (f0 + (i / CH) as i64) as f32 * 1e-6;
        }
        true
    };
    let mut out = vec![0f32; 1024 * CH];
    let mut local = 0i64;
    while local < len {
        p.read(local, &mut out, &mut pull);
        let t = (local + 1024) as f64 / len as f64;
        let expect = len as f64 * source_progress(1.0, &automation, t.min(1.0));
        let got = out[(1023) * CH] as f64 / 1e-6;
        // допуск — групповая задержка sinc-ресемплера (~3к кадров) плюс
        // квантование чанка; тот же порядок, что WARP_SOURCE_MARGIN_FRAMES
        // последний чанк выходит ЗА тело события — там источник кончился,
        // и сверять нечего (за краем клипа выход законно нулевой)
        if local + 1024 <= len {
            assert!(
                (got - expect).abs() < 4096.0,
                "t={t:.3}: позиция источника {got:.0} разошлась с интегралом {expect:.0}"
            );
        }
        local += 1024;
    }
}

#[test]
fn clip_processor_single_big_read_matches() {
    let automation = vec![
        (0.2, 1.0),
        (0.3, 0.35),
        (0.7, 0.6),
        (0.8, 1.0),
    ];
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        automation: vec![],
    }];
    let len = 20 * 48_000i64;
    let auto = profiles(&automation, len);
    let mut p = ClipProcessor::new(&fx, &auto, 0, len);
    let mut pull = |_k: kt_fx::PullKind, f0: i64, buf: &mut [f32]| {
        for (i, s) in buf.iter_mut().enumerate() {
            *s = (f0 + (i / CH) as i64) as f32 * 1e-6;
        }
        true
    };
    let mut out = vec![0f32; len as usize * CH];
    p.read(0, &mut out, &mut pull);
    let got = out[(len as usize - 1) * CH] as f64 / 1e-6;
    let expect = len as f64 * source_progress(1.0, &automation, 1.0);
    println!("single-read: got {got:.0}, expect {expect:.0}");
    assert!((got - expect).abs() < 4096.0, "got {got}, expect {expect}");
}
