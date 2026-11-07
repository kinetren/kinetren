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
}

impl Work {
    pub const ALL: [Work; 11] = [
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

/// The running totals, in the order of [`Work::ALL`].
pub fn totals() -> [u64; Work::ALL.len()] {
    std::array::from_fn(|i| COUNTS[i].load(Ordering::Relaxed))
}
