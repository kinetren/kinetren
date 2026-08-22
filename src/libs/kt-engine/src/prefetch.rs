//! Prefetch: идемпотентное вычисление недостающих burst'ов.
//!
//! Без состояния между тиками: wanted (lookahead playhead + видимый диапазон +
//! голова loop-региона) MINUS coverage(кэш) → submit в воркеры. Ключи строго
//! по сетке воспроизведения (в старом движке prefetch шёл своей сеткой 10/12
//! fps и наполнял кэш ключами, которые playback никогда не спрашивал —
//! дефект i).

use crate::audio::AudioRenderer;
use crate::worker::{DecodeRequest, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, BurstKind};
use kt_model::MediaId;
use std::collections::HashMap;

/// Один префетч-проход по аудио: досабмитить недостающее для [range) кадров.
/// Возвращает число отправленных запросов (для тестов/метрик).
pub fn prefetch_audio_pass(
    renderer: &AudioRenderer,
    cache: &BurstCache,
    workers: &HashMap<MediaId, MediaWorker>,
    range: std::ops::Range<i64>,
    prio: Priority,
    generation: u64,
) -> usize {
    let mut submitted = 0;
    for (media, bursts) in renderer.wanted_audio_bursts(range) {
        let Some(worker) = workers.get(&media) else { continue };
        let covered = cache.coverage(source_id(media), BurstKind::Audio, bursts.clone());
        let mut cursor = bursts.start;
        // идём по дыркам между covered-диапазонами
        for run in covered.iter().chain(std::iter::once(&(bursts.end..bursts.end))) {
            for idx in cursor..run.start {
                worker.submit(DecodeRequest { kind: RequestKind::Audio { idx }, prio, generation });
                submitted += 1;
            }
            cursor = run.end;
        }
    }
    submitted
}
