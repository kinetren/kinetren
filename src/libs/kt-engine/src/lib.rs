//! Движок: render graph (выбор событий, микс, композит), MediaWorker-планировщик
//! декода, transport (часы = аудио-колбэк), prefetch, Player-ядро.
//!
//! Реализовано: M2 (аудио-граф + transport + фетчеры), M3 (воркеры/prefetch),
//! M6-аудио (FX в графе), M4-ядро (Player + видео-композит).
//! Впереди: M7 (экспорт). Архитектура и владение потоками — docs/architecture.md.

pub mod flight;
pub mod audio;
pub mod export;
pub mod fetch;
pub mod fxstore;
pub mod player;
pub mod prefetch;
pub mod transport;
pub mod video;
pub mod video_plan;
pub mod worker;

pub use audio::{AudioRenderer, RenderStats, db_to_linear};
pub use export::{
    ExportBackends, ExportOptions, ExportStats, export_mp4, export_mp4_with,
    export_mp4_with_backends, gpu_required_clips, render_raw_audio, render_raw_video,
};
pub use flight::{FlightRecorder, Snap as FlightSnap};
pub use fetch::{AudioFetcher, CacheFetcher, DirectFetcher, source_id};
pub use fxstore::FxStateStore;
pub use player::{
    AudioShared, Player, VideoBackendStatus, callback_step, lookahead_windows, reverse_frames,
    varispeed_fill,
};
pub use prefetch::prefetch_audio_pass;
pub use transport::Transport;
pub use video::{VideoRenderer, compose_plan_cpu, video_cache_idx};
pub use video_plan::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, VideoGpuExec,
};
pub use worker::{DecodeRequest, MediaWorker, RequestKind};

/// Приоритеты запросов к MediaWorker. Порядок значим (инвариант №5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}
