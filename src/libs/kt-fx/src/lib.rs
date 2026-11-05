//! Audio FX: slowed (rubato-varispeed) + reverb (freeverb) + gain.
//!
//! Architectural decisions that close the crackles of the old engine
//! (defects a–f):
//! - all sound is f32 48kHz/2ch; FX do not see the source sample rate (a);
//! - the state lives in one object per clip, owned by the audio-render thread
//!   via FxStateStore in kt-engine, not shared with prefetch (b);
//! - varispeed is rubato SincFixedOut with continuous consumption of the source:
//!   a fractional cursor, no sample is skipped or duplicated (a, c, d);
//! - parameters are smoothed by one-pole (~20 ms) and NEVER reset
//!   the buffers; a full reset — only reset() at a continuity break (e);
//! - tail() reports the reverb tail — the graph extends the clip window (f).

use kt_time::AUDIO_CHANNELS;

pub const CH: usize = AUDIO_CHANNELS;

/// Pre-roll at a continuity break: max(tail, 100 ms) is rendered into garbage,
/// so that the state (reverb, resampler) is warmed up deterministically. Its one home is the
/// plugin API (the registry refuses a look-ahead that reaches it — plugin-api-stage1, D4).
pub use kt_plugin_api::MIN_PREROLL_FRAMES;

/// The role of an access to the source. `Warmup` — the pre-roll after a seek: these frames
/// are rendered into garbage and do not go to the output, so their cache miss is NOT an
/// underrun. It used to be one, and the counter grew at each playback start
/// and at each loop wrap-around (item 166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullKind {
    Output,
    Warmup,
}

pub mod conform;
pub mod geom;
pub mod mask;
pub mod plan;
pub mod simd;
pub mod video;

pub mod audio;

pub use audio::{
    AddrKnot, WARP_SOURCE_MARGIN_FRAMES, chain_addr_knots, chain_consumption_window_ext,
    chain_context, chain_fingerprint, chain_lookahead, chain_source_used, chain_source_window,
    chain_source_window_ext, chain_tail,
};
pub use audio::{
    ChainOpts, ClipProcessor, Freeverb, SmoothedParam, TrackProcessor, Warp,
};

/// The time-stretch map MOVED TO kt-model (R.39): it must be called by
/// `clip_slice` — the clip-slice rule — and kt-model cannot depend on kt-fx.
/// Here there is only a re-export: the formula still has ONE home, and the engine
/// and the tests keep calling `kt_fx::…`; not a single call has been touched.
pub use kt_model::{
    AutoProfiles, ParamPoints, RATE_AUDIO_FLOOR, RATE_CLAMP, SignKnot, avg_rate, chain_avg_rate,
    chain_rate, consumed_abs, eff_rate, output_time_for_consumption, output_time_for_source,
    param_at, param_integral, rate_at, rate_reach, rate_signed, sign_knots, source_progress,
    span_over, warp_material_ticks, warp_span_ticks,
};
