//! Тесты 403-405 (Р.49 Б4): громкость по BS.1770 / EBU R 128.

use kt_fx::audio::loudness::{LoudnessMeter, measure};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

/// Синус заданной амплитуды и длины.
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

/// Тест 403 (Р.49 Б4): КАЛИБРОВКА шкалы.
///
/// Формула BS.1770: `L = −0.691 + 10·log10(Σ G_i · z_i)`, где `z_i` — средний
/// квадрат K-фильтрованного канала. Смещение −0.691 выбрано не случайно: оно
/// в точности гасит усиление K-фильтра на 1 кГц для стереопары, и синус 1 кГц
/// полной шкалы, поданный в оба канала, читается как 0.0 LUFS. Отсюда и
/// проверка: тон −20 dBFS обязан дать −20.0 LUFS.
///
/// Допуск EBU — 0.1 LU, и это причина, по которой K-фильтр считается в f64:
/// коэффициент `-1.99004745483398` в f32 обрезается, а биквад РЕКУРСИВЕН —
/// ошибка полюса копится по всему файлу.
#[test]
fn t403_scale_is_calibrated() {
    // полная шкала в оба канала — ноль по определению калибровки
    let full = measure(&sine(1000.0, 1.0, 20.0)).lufs.expect("тон не может быть ниже гейта");
    assert!(
        full.abs() <= 0.1,
        "синус 1 кГц полной шкалы в стереопаре обязан дать 0.0 LUFS ±0.1, получено {full:.3}"
    );

    // −20 dBFS: амплитуда 0.1; 20 секунд — достаточно блоков для гейтов
    let amp = 10f32.powf(-20.0 / 20.0);
    let r = measure(&sine(1000.0, amp, 20.0));
    let lufs = r.lufs.expect("тон не может быть ниже абсолютного гейта");
    assert!(
        (lufs + 20.0).abs() <= 0.1,
        "тон −20 dBFS обязан дать −20.0 LUFS ±0.1, получено {lufs:.3}"
    );
    // тот же тон на 10 dB тише — ровно на 10 LU тише
    let quiet = measure(&sine(1000.0, amp * 10f32.powf(-10.0 / 20.0), 20.0));
    let q = quiet.lufs.unwrap();
    assert!(
        ((lufs - q) - 10.0).abs() <= 0.1,
        "шкала не линейна: {lufs:.3} против {q:.3}"
    );
    // ровный тон — это НУЛЕВОЙ диапазон громкости
    assert!(r.lra <= 1.0, "у ровного тона LRA обязан быть около нуля, получено {:.2}", r.lra);

    // тишина — не «−70», а «числа нет»: врать о громкости, которой не
    // существует, хуже, чем честно молчать
    let silence = vec![0.0f32; AUDIO_SAMPLE_RATE as usize * AUDIO_CHANNELS];
    assert_eq!(measure(&silence).lufs, None);
}

/// Причина, по которой громкость считается в f64, — СУММЫ, а не
/// коэффициенты. Тест сторожит саму причину: он показывает, что прямое
/// накопление квадратов в f32 уезжает за допуск EBU (0.1 LU), и значит
/// перевод накопителей в одинарную точность «ради скорости» — регресс, а не
/// оптимизация.
#[test]
fn t403b_f32_accumulator_drifts_past_the_ebu_tolerance() {
    let sr = AUDIO_SAMPLE_RATE as usize;
    let n = sr * 600; // десять минут
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
        "накопление в f32 уложилось в допуск ({drift:.3} LU) — значит в модульном \
         комментарии написана неправда и причину для f64 надо назвать другую"
    );
    // а сам метр, считающий в f64, на том же материале точен
    let r = measure(&sine(1000.0, amp as f32, 60.0)).lufs.unwrap();
    assert!((r + 20.0).abs() <= 0.1, "метр в f64 уехал: {r:.3} LUFS");
}

/// Тест 404 (Р.49 Б4): МЕЖСЭМПЛОВЫЙ выброс виден.
///
/// Между сэмплами восстановленная волна проходит ВЫШЕ, чем в них: синус на
/// четверти частоты дискретизации, сдвинутый по фазе на 45°, имеет сэмплы
/// ровно на ±0.707·A, а настоящий пик — A. Пик по сэмплам сказал бы
/// «−3 dBFS, запас есть», а после кодека получился бы клиппинг.
#[test]
fn t404_true_peak_sees_the_intersample_overshoot() {
    let n = AUDIO_SAMPLE_RATE as usize * 2;
    let mut v = Vec::with_capacity(n * AUDIO_CHANNELS);
    let mut sample_peak = 0.0f32;
    for i in 0..n {
        // 12 кГц при 48 кГц = четверть частоты; фаза 45° ставит сэмплы
        // в точки ±1/√2, а максимум волны — между ними
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
        "сцена: пик по сэмплам обязан быть около −3 dBFS, получено {:.2}",
        db(sample_peak)
    );
    assert!(
        r.dbtp > db(sample_peak) + 1.5,
        "истинный пик не увидел межсэмплового выброса: {:.2} dBTP против {:.2} dBFS по сэмплам",
        r.dbtp,
        db(sample_peak)
    );
    assert!(r.dbtp <= 0.5, "истинный пик завышен: {:.2} dBTP при максимуме волны 0 dBFS", r.dbtp);
}

/// Тест 405 (Р.49 Б4): НАРЕЗКА не влияет на результат.
///
/// Метр кормится тем, чем кормится: кусками экспорта, burst'ами пирамиды,
/// целым буфером. Если бы сетка блоков или фильтры жили в вызове, а не в
/// накопителе, одно и то же аудио давало бы разные числа в зависимости от
/// того, кто его подал.
#[test]
fn t405_chunking_does_not_change_the_result() {
    // материал с ТИШИНОЙ и громкими кусками: гейты обязаны сработать
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
            "нарезка по {chunk_frames} кадров изменила громкость: {a} против {b}"
        );
        assert!((whole.dbtp - got.dbtp).abs() < 1e-9, "нарезка изменила истинный пик");
        assert!((whole.lra - got.lra).abs() < 1e-9, "нарезка изменила диапазон");
    }

    // и гейты работают: паузы не утянули результат вниз к −70
    // половина материала — тишина. Без гейтов средняя энергия была бы вдвое
    // меньше, то есть около −23 LUFS; с гейтами результат обязан остаться у
    // громкости самих фраз. Полградуса разницы дают блоки НА СТЫКЕ тона и
    // тишины — они выше относительного гейта и входят в счёт законно.
    let l = whole.lufs.unwrap();
    assert!(
        l > -21.5,
        "паузы утянули громкость до {l:.2} LUFS — гейты не сработали (без них было бы \
         около −23, с ними ожидается около −20)"
    );
    assert!(l < -19.5, "громкость завышена: {l:.2} LUFS при фразах на −20");
}
