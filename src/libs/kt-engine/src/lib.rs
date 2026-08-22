//! Движок: render graph (выбор событий, микс, композит), MediaWorker-планировщик
//! декода, transport (часы = аудио-колбэк), prefetch, экспорт.
//!
//! Реализация: M2 (граф+transport на синтетике), M3 (воркеры/prefetch),
//! M7 (экспорт). Архитектура и владение потоками — docs/architecture.md.

/// Приоритеты запросов к MediaWorker. Порядок значим (инвариант №5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}
