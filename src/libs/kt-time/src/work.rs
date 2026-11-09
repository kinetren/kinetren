//! Work counters (idle-cpu; the rule "the program at rest costs nothing" in `CLAUDE.md`): how many
//! times the program did each kind of job, process-wide. Percent of a processor depends on the
//! machine; "composites on a pause" does not, and at rest every one of these stands still.
//!
//! A relaxed atomic add and nothing else: a counter must cost less than the guard it feeds, and
//! it steers nothing (invariant 11). It lives here because every crate that does the work depends
//! on this one.

use std::sync::atomic::{AtomicU64, Ordering};

/// One kind of job. Append only: the load report names them by [`Work::name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Work {
    /// A UI frame ran.
    UiFrame,
    /// The preview's composite was asked of the executor.
    Composite,
    /// A command buffer went to a device, from any engine.
    DeviceSubmit,
    /// A request reached a decoder's queue.
    DecodeRequest,
    /// A decoder delivered a picture or an audio burst.
    Decoded,
    /// The prefetch thread ran a pass.
    PrefetchPass,
    /// The audio render thread filled the ring.
    AudioRender,
    /// A file thumbnail was ordered.
    ThumbOrder,
    /// An effect picker cell was rendered.
    FxThumb,
    /// A composition's preview job started.
    ReelJob,
    /// The timeline's events were laid out and painted.
    EventPass,
    /// The model took a real edit (input-without-work: a command that changed nothing is none).
    ModelEdit,
    /// The player took a whole new project: a clone and a wake of both engine threads.
    ProjectSwap,
    /// An audio or video renderer was built for a project: the price every swap pays on the
    /// audio render thread, the prefetch thread and the UI. On the owner's project of 1974
    /// events one audio build cost 447 ms; it ran on every mouse move and no counter saw it.
    EngineBuild,
    /// The audio render thread handed out a burst of the stored final mix (cached-range-plays):
    /// no decode, no effect ran for it.
    MixReplay,
    /// The preview showed a stored composite (cached-range-plays): no layer was composed.
    CompositeReplay,
    /// The scheduler started a job (one-scheduler): a thumbnail batch, an effect cell, a poster, a
    /// reel strip, an analysis, an index pass, a save, a media open.
    SchedJob,
}

impl Work {
    pub const ALL: [Work; 17] = [
        Work::UiFrame,
        Work::Composite,
        Work::DeviceSubmit,
        Work::DecodeRequest,
        Work::Decoded,
        Work::PrefetchPass,
        Work::AudioRender,
        Work::ThumbOrder,
        Work::FxThumb,
        Work::ReelJob,
        Work::EventPass,
        Work::ModelEdit,
        Work::ProjectSwap,
        Work::EngineBuild,
        Work::MixReplay,
        Work::CompositeReplay,
        Work::SchedJob,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Work::UiFrame => "ui_frames",
            Work::Composite => "composites",
            Work::DeviceSubmit => "device_submits",
            Work::DecodeRequest => "decode_requests",
            Work::Decoded => "decoded",
            Work::PrefetchPass => "prefetch_passes",
            Work::AudioRender => "audio_renders",
            Work::ThumbOrder => "thumb_orders",
            Work::FxThumb => "fx_thumbs",
            Work::ReelJob => "reel_jobs",
            Work::EventPass => "event_passes",
            Work::ModelEdit => "model_edits",
            Work::ProjectSwap => "project_swaps",
            Work::EngineBuild => "engine_builds",
            Work::MixReplay => "mix_replays",
            Work::CompositeReplay => "composite_replays",
            Work::SchedJob => "sched_jobs",
        }
    }
}

static COUNTS: [AtomicU64; Work::ALL.len()] = [const { AtomicU64::new(0) }; Work::ALL.len()];

#[inline]
pub fn bump(w: Work) {
    add(w, 1);
}

#[inline]
pub fn add(w: Work, n: u64) {
    COUNTS[w as usize].fetch_add(n, Ordering::Relaxed);
}

/// The running total of one kind.
pub fn count(w: Work) -> u64 {
    COUNTS[w as usize].load(Ordering::Relaxed)
}

/// The running totals, in the order of [`Work::ALL`].
pub fn totals() -> [u64; Work::ALL.len()] {
    std::array::from_fn(|i| COUNTS[i].load(Ordering::Relaxed))
}
