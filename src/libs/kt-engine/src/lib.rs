//! Движок: render graph (выбор событий, микс, композит), MediaWorker-планировщик
//! декода, transport (часы = аудио-колбэк), prefetch, экспорт.
//!
//! Реализовано: M2 (аудио-граф + transport + фетчеры на синтетике).
//! Впереди: M3 (MediaWorker/prefetch-поток), M6 (FX в графе), M7 (экспорт).
//! Архитектура и владение потоками — docs/architecture.md.

pub mod audio;
pub mod fetch;
pub mod transport;

pub use audio::{AudioRenderer, RenderStats, db_to_linear};
pub use fetch::{AudioFetcher, CacheFetcher, DirectFetcher, source_id};
pub use transport::Transport;

/// Приоритеты запросов к MediaWorker. Порядок значим (инвариант №5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}
