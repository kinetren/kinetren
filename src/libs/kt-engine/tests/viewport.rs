//! Тест 13 (docs/porting-notes.md): скролл/зум таймлайна не порождает
//! декод-запросов (инвариант №8). Старый движок на каждый сдвиг вьюпорта
//! декодировал тумбнейлы/волноформы заново (дефект m).
//!
//! Модель UI-отрисовки: волноформы = чтение peak-пирамиды (память/БД),
//! полоски прогретости = cache.coverage(). Здесь мы прогоняем сотни
//! «перерисовок» случайных вьюпортов и проверяем счётчики обращений
//! к источнику.

use kt_cache::{BurstCache, BurstKind, CacheConfig};
use kt_media::index::PeakPyramid;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::Tick;
use std::sync::Arc;

#[test]
fn t13_viewport_churn_causes_zero_source_reads() {
    let mut src = SyntheticSource::new(Tick(90_000 * 30), AudioPattern::Sine { hz: 440.0 });

    // индекс построен один раз (это единственные разрешённые чтения)
    let pyramid = PeakPyramid::build(&mut src);
    let reads_after_index = src.audio_reads;
    assert!(reads_after_index > 0);

    // кэш частично прогрет (как после prefetch)
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    for idx in 0..64 {
        cache.insert_audio(kt_cache::SourceId(1), Arc::new(src.read_audio_burst(idx).unwrap()));
    }
    let reads_after_prefetch = src.audio_reads;

    // 500 «перерисовок» случайных вьюпортов: выбор уровня пирамиды по зуму,
    // чтение блоков min/max, запрос coverage — ровно то, что делает
    // kt-app::draw_timeline
    let mut acc = 0i64; // защита от выкидывания цикла оптимизатором
    let mut state = 0x9e3779b97f4a7c15u64; // детерминированный LCG
    for _ in 0..500 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let zoom_level = (state >> 32) as usize % pyramid.levels.len();
        let start_block = (state as usize) % pyramid.levels[zoom_level][0].len().max(1);
        let blocks = &pyramid.levels[zoom_level][0];
        for b in blocks.iter().skip(start_block).take(200) {
            acc += b.0 as i64 + b.1 as i64;
        }
        let cov = cache.coverage(kt_cache::SourceId(1), BurstKind::Audio, 0..(state % 100) as i64);
        acc += cov.len() as i64;
        // выбор уровня как в UI
        let frames_per_px = 1.0 + (state % 10_000) as f64;
        let mut level = 0usize;
        let mut block = PEAK_BLOCK_FRAMES as f64;
        while block * 2.0 <= frames_per_px && level + 1 < pyramid.levels.len() {
            level += 1;
            block *= 2.0;
        }
        acc += level as i64;
    }
    assert!(acc != 0);

    assert_eq!(
        src.audio_reads, reads_after_prefetch,
        "перерисовка вьюпорта не имеет права трогать источник (инвариант №8)"
    );
}
