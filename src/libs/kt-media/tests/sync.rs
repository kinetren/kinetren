//! Тесты 413-415 (Р.49 Б8): сдвиг двух записей по звуку.

use kt_media::PEAK_BLOCK_FRAMES;
use kt_media::sync::{SYNC_GRANULARITY_TICKS, offset_from_pyramids};
use kt_time::AUDIO_SAMPLE_RATE;

/// Детерминированный «шум» без внешних зависимостей.
fn rng(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32 / (1u64 << 31) as f32) - 1.0
}

/// Сцена: удары в заданные секунды, с заданным «цветом» и уровнем.
/// `lowpass` грубо имитирует другой микрофон (камера в глубине комнаты),
/// `noise` — шум зала.
fn take(hits: &[f64], secs: f64, lowpass: bool, level: f32, noise: f32, seed0: u64) -> Vec<f32> {
    let sr = AUDIO_SAMPLE_RATE as usize;
    let n = (sr as f64 * secs) as usize;
    let mut v = vec![0.0f32; n];
    let mut seed = seed0;
    for &h in hits {
        let at = (h * sr as f64) as usize;
        // удар: резкая атака, экспоненциальный спад 150 мс
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
        // однополюсный НЧ — «другой микрофон»: форма огибающей меняется
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

/// Пирамида из моно-сигнала: ровно та же арифметика, что у
/// `PeakPyramid::build` (блок — минимум и максимум, уровень выше — попарное
/// объединение). Строится здесь, а не источником, чтобы тест не зависел от
/// набора паттернов синтетики.
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

/// Тест 413 (Р.49 Б8): сдвиг находится, даже когда записи звучат по-разному.
/// Микрофоны разные: у одного АЧХ завалена, уровень вдвое ниже, и добавлен
/// шум зала. Общего у них только МОМЕНТЫ событий — на них и считается.
#[test]
fn t413_finds_the_offset_across_different_microphones() {
    let hits = [0.7, 1.9, 2.4, 3.6, 4.1, 5.5, 6.2];
    let shift = 0.4; // B отстаёт на 400 мс
    let a = take(&hits, 8.0, false, 0.8, 0.0, 1);
    let b_hits: Vec<f64> = hits.iter().map(|h| h + shift).collect();
    let b = take(&b_hits, 8.0, true, 0.35, 0.02, 7);

    let r = offset_from_pyramids(&pyramid(&a), &pyramid(&b), PEAK_BLOCK_FRAMES)
        .expect("сдвиг обязан найтись");
    let want = (shift * 90_000.0) as i64;
    let block_ticks = (PEAK_BLOCK_FRAMES as i64 * 90_000) / AUDIO_SAMPLE_RATE as i64;
    assert!(
        (r.ticks.0 - want).abs() <= block_ticks,
        "сдвиг {} тиков против ожидаемых {want} (допуск — блок пирамиды, {block_ticks})",
        r.ticks.0
    );
    assert!(r.confidence > 0.3, "уверенность {} слишком мала", r.confidence);
    assert_eq!(r.ticks.0 % SYNC_GRANULARITY_TICKS, 0, "результат не лёг на сетку точности");
}

/// Тест 414 (Р.49 Б8): ПЕРЕСТАНОВКА аргументов даёт ровно минус тот же сдвиг.
///
/// Классическая ошибка корреляции: знак лага теряется, и «подвинь второй файл
/// на +0.4 с» превращается в «подвинь на −0.4 с». Монтажёр в этом случае
/// разводит дорожки вдвое дальше друг от друга, чем они были.
#[test]
fn t414_swapping_arguments_flips_the_sign() {
    let hits = [0.5, 1.4, 2.8, 3.3, 4.9, 5.6];
    let a = take(&hits, 7.0, false, 0.8, 0.0, 3);
    let b_hits: Vec<f64> = hits.iter().map(|h| h + 0.3).collect();
    let b = take(&b_hits, 7.0, false, 0.8, 0.0, 3);
    let (pa, pb) = (pyramid(&a), pyramid(&b));

    let ab = offset_from_pyramids(&pa, &pb, PEAK_BLOCK_FRAMES).expect("прямой порядок");
    let ba = offset_from_pyramids(&pb, &pa, PEAK_BLOCK_FRAMES).expect("обратный порядок");
    assert!(ab.ticks.0 > 0, "B отстаёт — сдвиг обязан быть положительным, получено {}", ab.ticks.0);
    assert_eq!(
        ab.ticks.0, -ba.ticks.0,
        "перестановка аргументов не дала минус тот же сдвиг: {} и {}",
        ab.ticks.0, ba.ticks.0
    );
    // и повтор даёт то же самое: тай-брейк детерминирован
    let again = offset_from_pyramids(&pa, &pb, PEAK_BLOCK_FRAMES).unwrap();
    assert_eq!(ab.ticks, again.ticks);
    assert_eq!(ab.confidence, again.confidence);
}

/// Тест 415 (Р.49 Б8): НЕСВЯЗАННЫЕ записи дают отказ, а не ноль.
///
/// Ответ «сдвиг 0» неотличим от «они и так совпадают», и монтажёр поверит.
/// Сдвиг на глазок хуже отказа: он подвинет дорожку, услышит фленжер и будет
/// искать причину в чём угодно, кроме автоматики.
#[test]
fn t415_unrelated_takes_are_refused_not_guessed() {
    let a = take(&[0.6, 1.7, 2.9, 4.4], 6.0, false, 0.8, 0.02, 11);
    // другие события в другие моменты — общего ничего
    let b = take(&[0.2, 2.2, 3.1, 5.2], 6.0, false, 0.8, 0.02, 99);
    let r = offset_from_pyramids(&pyramid(&a), &pyramid(&b), PEAK_BLOCK_FRAMES);
    assert!(
        r.is_none() || r.unwrap().confidence <= 0.6,
        "у несвязанных записей найден уверенный сдвиг {r:?} — программа выдумывает"
    );

    // чистый шум против чистого шума — отказ
    let mut s1 = 5u64;
    let mut s2 = 500u64;
    let n = AUDIO_SAMPLE_RATE as usize * 4;
    let n1: Vec<f32> = (0..n).map(|_| rng(&mut s1) * 0.3).collect();
    let n2: Vec<f32> = (0..n).map(|_| rng(&mut s2) * 0.3).collect();
    let r = offset_from_pyramids(&pyramid(&n1), &pyramid(&n2), PEAK_BLOCK_FRAMES);
    assert!(r.is_none(), "у двух разных шумов найден сдвиг: {r:?}");
}
