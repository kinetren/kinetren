//! Prefetch: idempotent computation of the missing bursts.
//!
//! Stateless between ticks: wanted (playhead lookahead + the visible range +
//! the loop region's head) MINUS coverage(cache) → submit to the workers. Keys strictly
//! on the playback grid (in the old engine prefetch ran on its own 10/12
//! fps grid and filled the cache with keys that playback never asked for —
//! defect i).

use crate::audio::AudioRenderer;
use crate::worker::{DecodeRequest, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, BurstKind};
use kt_model::MediaId;
use std::collections::HashMap;

/// One prefetch pass over the audio: resubmit what is missing for [range) frames.
/// Returns the number of requests sent (for tests/metrics).
pub fn prefetch_audio_pass(
    renderer: &AudioRenderer,
    cache: &BurstCache,
    workers: &HashMap<MediaId, MediaWorker>,
    range: std::ops::Range<i64>,
    prio: Priority,
    generation: u64,
    // The raw (reverse-transport) render path: the source window is computed
    // linearly, as `render_plain_clip` reads it (R.43, item 295).
    raw: bool,
) -> usize {
    let mut submitted = 0;
    let wanted = if raw {
        renderer.wanted_audio_bursts_raw(range)
    } else {
        renderer.wanted_audio_bursts(range)
    };
    for (media, bursts) in wanted {
        let Some(worker) = workers.get(&media) else { continue };
        let covered = cache.coverage(source_id(media), BurstKind::Audio, bursts.clone());
        let mut cursor = bursts.start;
        // walk the gaps between covered ranges
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
