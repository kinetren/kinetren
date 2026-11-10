//! Engine: render graph (event selection, mix, composite), MediaWorker scheduler
//! for decode, transport (the clock = the audio callback), prefetch, the Player core.
//!
//! Implemented: M2 (audio graph + transport + fetchers), M3 (workers/prefetch),
//! M6-audio (FX in the graph), M4-core (Player + video composite).
//! Ahead: M7 (export). Architecture and thread ownership — docs/architecture.md.

pub mod audio;
pub mod builds;
pub mod export;
pub mod export_pipeline;
pub mod fetch;
pub mod flight;
pub mod mix;
pub mod fxstore;
pub mod incremental;
pub mod compose_queue;
pub mod player;
pub mod prefetch;
pub mod prewarm;
pub mod preview_bench;
pub mod transitions;
pub mod transport;
pub mod video;
pub mod video_plan;

pub mod worker;

pub use audio::{AudioRenderer, RenderStats, db_to_linear, event_gain_linear};
pub use export::{
    ExportBackends, ExportOptions, ExportStats,
    export_mp4_with_backends, render_raw_audio, stream_preroll,
};
pub use fetch::{AudioFetcher, CacheFetcher, DirectFetcher, source_id};
pub use flight::{FlightRecorder, Snap as FlightSnap};
pub use fxstore::FxStateStore;
pub use player::{
    AudioShared, Player, Sources, VideoBackendStatus, VideoCost, callback_step, hold_builds_for_tests,
    hold_start_bursts_for_tests,
    lookahead_windows,
    reverse_frames, varispeed_fill,
};
pub use prefetch::{
    FramesSizing, PREFETCH_VIDEO_LOOKAHEAD_TICKS, frames_cache_want, loop_frame_bytes, prefetch_audio_pass,
    simulate_video_playback, size_frames_cache, video_prefetch_pass, video_prefetch_pass_except,
    video_prefetch_pass_sized, video_prefetch_wants, video_prefetch_wants_except,
};
pub use transitions::TransMix;
pub use transport::Transport;
pub use video::{
    NestKey, SourceFacts, StreamPacing, VideoRenderer, clip_source_addr, stop_when, clip_source_addr_in,
    content_hash, gen_salt, gen_source_addr, gen_source_addr_in, gen_sources, instance_content,
    nest_source_id, reel_chain_of_clip, resolve_cam3d, revisions_scope, video_cache_idx,
};
pub use video_plan::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, GpuStages, LayerSrc, PlanLayer, PresentOptions,
    VideoGpuExec, chain_plan,
};
pub use worker::{DecodeRequest, MediaWorker, RequestKind, Retune};

/// Priorities of requests to the MediaWorker. The order matters (invariant 5).
/// Time stretching of the clip for FOREIGN consumers (R.37 item 3).
///
/// The timeline preview — waveform and filmstrip — must compute the position of
/// the material with THE SAME map as the render: otherwise a slowed clip draws
/// not what it plays. The formula lives here, not in kt-app, by the same rule
/// as the rest of the geometry: do not create a formula in the UI.
///
/// This cannot be built for every pixel column — the caller must
/// cache it (in kt-app this is `warp_maps()` by `model_rev`).
/// The map moved to kt-model (R.39): the constructor is ONE per render, preview
/// and the clip's chunk rule. Here is a re-export — the calls in kt-app are untouched.
pub use kt_model::{ClipWarp, clip_warp as clip_warp_map};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}
