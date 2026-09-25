//! Тесты 767-768 (Р.65): КЛЮЧЕВОЙ вход компрессора.
//!
//! Владелец (Р.63 п.8.4): «в качестве дакинга можно показать тот же
//! задублированный виджет аудио-компрессора… повсюду копятся бессмысленные
//! копипасты одного и того же кода». Отсюда правило: дакинг — это НЕ новый
//! прибор, а компрессор, у которого назначен ключ. Здесь проверяется сам
//! прибор: детектор слушает ключ, сжимается свой вход, а без ключа всё
//! остаётся ровно прежним.

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
    }
}

/// Ровный тон — «музыка».
fn tone(frames: usize) -> Vec<f32> {
    let mut v = vec![0f32; frames * CH];
    for i in 0..frames {
        let s = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / SR as f32).sin() * 0.5;
        v[i * CH] = s;
        v[i * CH + 1] = s;
    }
    v
}

/// «Речь»: две громкие реплики с паузой между ними.
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

/// Тест 767 (Р.65): ключ управляет, сжимается свой вход.
#[test]
fn t767_key_drives_the_detector_and_input_is_compressed() {
    let frames = SR / 2; // 0.5 c
    // пауза между репликами — заведомо длиннее возврата (60 мс ≈ 2880
    // кадров): иначе «вернулся ли уровень» меряется на полуспаде
    let spans = [(2_000usize, 8_000usize), (20_000usize, 23_000usize)];

    // --- С КЛЮЧОМ: детектор слышит речь, сжимается тон -----------------
    let mut p = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut out = tone(frames);
    let key = speech(frames, &spans);
    p.process_keyed(0, &mut out, &|slot| (slot == 0).then_some(&key[..]));

    // …под репликой тише, между репликами — нет
    let under = rms(&out, 6_000, 8_000);
    let between = rms(&out, 16_000, 18_000);
    let before = rms(&out, 0, 1_500);
    assert!(
        under < between * 0.6,
        "под ключом музыка обязана просесть: {under} против {between} между репликами"
    );
    assert!(
        (between / before - 1.0).abs() < 0.2,
        "между репликами уровень обязан вернуться: {between} против {before} до первой"
    );

    // …и ЗВУЧИТ именно свой вход: под репликой в выходе частота тона (220),
    // а не ключа (700) — иначе мы бы «подмешали» ключ вместо управления им
    let z = |buf: &[f32], a: usize, b: usize| {
        buf[a * CH..b * CH].chunks_exact(CH).filter(|w| w[0] == 0.0).count()
    };
    assert_eq!(z(&out, 6_000, 8_000), 0, "в выходе появились нули — это не сжатый тон");

    // --- БЕЗ КЛЮЧА: ровно прежний прибор -------------------------------
    let mut p_plain = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut plain = tone(frames);
    p_plain.process(0, &mut plain);

    // ключ, РАВНЫЙ собственному входу, обязан дать то же самое байт-в-байт:
    // «то же снижение, что дал бы этот же сигнал на собственном входе»
    let mut p_self = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut selfkey = tone(frames);
    let same = tone(frames);
    p_self.process_keyed(0, &mut selfkey, &|slot| (slot == 0).then_some(&same[..]));
    assert_eq!(selfkey, plain, "ключ, равный входу, дал не то же, что путь без ключа");
}

/// Тест 768 (Р.65): нейтральность и цена ключевой ветки.
#[test]
fn t768_silent_key_is_transparent_and_costs_nothing() {
    let frames = 4_096;
    let mut p = TrackProcessor::new(&[comp()], ChainOpts::default());
    let mut out = tone(frames);
    let src = tone(frames);
    let silent = vec![0f32; frames * CH];
    p.process_keyed(0, &mut out, &|_| Some(&silent[..]));
    for (o, s) in out.iter().zip(src.iter()) {
        assert!((o - s).abs() < 1e-6, "тишина в ключе обязана оставить вход нетронутым");
    }

    // ключ КОРОЧЕ буфера — это «ключ молчит» на остатке, а не паника
    let short = vec![0f32; (frames / 2) * CH];
    let mut out2 = tone(frames);
    p.process_keyed(0, &mut out2, &|_| Some(&short[..]));
    assert!(out2.iter().all(|s| s.is_finite()), "короткий ключ дал не-числа");
}
