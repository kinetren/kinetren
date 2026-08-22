//! Диагностика: позиция источника на выходе vs интеграл профиля.
use kt_fx::{CH, ClipProcessor, rate_integral};
use kt_model::{FxInstance, SlowedKeyframe};

#[test]
fn clip_processor_follows_integral_map() {
    let automation = vec![
        SlowedKeyframe { t: 0.2, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.3, rate: 0.35, mix: 0.0 },
        SlowedKeyframe { t: 0.7, rate: 0.6, mix: 0.0 },
        SlowedKeyframe { t: 0.8, rate: 1.0, mix: 0.0 },
    ];
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        automation: automation.clone(),
    }];
    let len = 20 * 48_000i64;
    let mut p = ClipProcessor::new(&fx, 0, len);
    // источник = ramp (значение = номер кадра * 1e-6)
    let mut pull = |f0: i64, buf: &mut [f32]| {
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
        let expect = len as f64 * rate_integral(1.0, &automation, t.min(1.0));
        let got = out[(1023) * CH] as f64 / 1e-6;
        if (local / 1024) % 200 == 0 || local + 1024 >= len {
            println!("t={t:.3} src_got={got:.0} src_expect={expect:.0} diff={:.0}", got - expect);
        }
        local += 1024;
    }
}

#[test]
fn clip_processor_single_big_read_matches() {
    let automation = vec![
        SlowedKeyframe { t: 0.2, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.3, rate: 0.35, mix: 0.0 },
        SlowedKeyframe { t: 0.7, rate: 0.6, mix: 0.0 },
        SlowedKeyframe { t: 0.8, rate: 1.0, mix: 0.0 },
    ];
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        automation: automation.clone(),
    }];
    let len = 20 * 48_000i64;
    let mut p = ClipProcessor::new(&fx, 0, len);
    let mut pull = |f0: i64, buf: &mut [f32]| {
        for (i, s) in buf.iter_mut().enumerate() {
            *s = (f0 + (i / CH) as i64) as f32 * 1e-6;
        }
        true
    };
    let mut out = vec![0f32; len as usize * CH];
    p.read(0, &mut out, &mut pull);
    let got = out[(len as usize - 1) * CH] as f64 / 1e-6;
    let expect = len as f64 * rate_integral(1.0, &automation, 1.0);
    println!("single-read: got {got:.0}, expect {expect:.0}");
    assert!((got - expect).abs() < 4096.0, "got {got}, expect {expect}");
}
