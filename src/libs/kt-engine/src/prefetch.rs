//! Prefetch: idempotent computation of the missing bursts.
//!
//! Stateless between ticks: wanted (playhead lookahead + the visible range +
//! the loop region's head) MINUS coverage(cache) → submit to the workers. Keys strictly
//! on the playback grid (in the old engine prefetch ran on its own 10/12
//! fps grid and filled the cache with keys that playback never asked for —
//! defect i).

use crate::audio::AudioRenderer;
use crate::player::lookahead_windows;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::worker::{DecodeRequest, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, BurstKey, BurstKind};
use kt_media::Quality;
use kt_model::MediaId;
use kt_time::Tick;
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

/// The video look-ahead cap: 2 s past the playhead.
pub const PREFETCH_VIDEO_LOOKAHEAD_TICKS: i64 = 2 * kt_time::TICK_HZ;

/// The bytes of one decoded frame at a decode quality (RGBA).
fn frame_bytes(q: Quality) -> usize {
    q.width as usize * q.height as usize * 4
}

/// The frame keys one video prefetch pass asks for at playhead `t0`, NEAREST FIRST: the
/// look-ahead window (folded into the loop region, `lookahead_windows`) at the preview
/// quality `q`, at most 2 s and at most HALF the video budget in bytes.
///
/// The window is measured by what it asks for — every layer of every frame at its own
/// decode size — and is cut at a whole frame (video-prefetch-thrash). Before, its length
/// was the half budget divided by ONE frame of the largest decode quality (item 19b,
/// R.46 item 318): a stack of layers (a cut over its blurred backdrop, a floating frame,
/// a reel) asked for several frames per tick, and on the showcase edit one pass wanted
/// 604 MB of a 512 MB cache, so the cache dropped the window's own nearest frames before
/// they were read. The half is the room for what playback reads meanwhile (the composite
/// memo, the frames just shown); `video_prefetch_pass` keeps the window ahead of those.
///
/// The playhead's own frame is always asked for, whatever its size: without it nothing
/// would be shown at all. Past the bytes, up to the same 2 s, come the first frames of the
/// cuts ahead (`jumps_to`): a 1080p window is 0.7 s long, and on the showcase a 4K file took
/// 0.85 s to reach an event's first frame, so that cut was late however fast everything was.
pub fn video_prefetch_wants(
    video: &VideoRenderer,
    t0: Tick,
    q: Quality,
    region_ticks: Option<(i64, i64)>,
    max_video_bytes: usize,
) -> Vec<(MediaId, Tick, Quality)> {
    let step = video.grid.frame_step().max(1);
    let share = max_video_bytes / 2;
    let mut out: Vec<(MediaId, Tick, Quality)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut bytes = 0usize;
    // the bytes ran out: from here on only what a decoder must jump to
    let mut full = false;
    let mut leads = 0usize;
    let mut before: Vec<(MediaId, Tick, Quality)> = Vec::new();
    for r in lookahead_windows(t0.0, PREFETCH_VIDEO_LOOKAHEAD_TICKS, region_ticks) {
        let mut t = video.grid.align_floor(Tick(r.start));
        while t.0 < r.end {
            let layers = video.layers_at(t, q);
            // a source slower than the project grid repeats its frame, and a blended
            // pair shares a side with the next tick: a key is counted once
            let frame: Vec<_> = layers.iter().filter(|k| !seen.contains(*k)).copied().collect();
            if !full {
                let fb: usize = frame.iter().map(|(_, _, dq)| frame_bytes(*dq)).sum();
                full = bytes + fb > share && !out.is_empty();
            }
            if !full {
                bytes += frame.iter().map(|(_, _, dq)| frame_bytes(*dq)).sum::<usize>();
                seen.extend(frame.iter().copied());
                out.extend(frame);
            } else {
                for k in frame.into_iter().filter(|k| jumps_to(k, &before, step)) {
                    if leads == CUT_LEAD_KEYS {
                        return out;
                    }
                    leads += 1;
                    seen.insert(k);
                    out.push(k);
                }
            }
            before = layers;
            t = Tick(t.0 + step);
        }
    }
    out
}

/// Keys past the window's bytes that one pass may still ask: two sizes at each of three cuts.
const CUT_LEAD_KEYS: usize = 6;

/// Does this layer of a moment start a read the moment before was not doing? The decoder has
/// to seek and decode up to it, which takes the same time however near the playhead it is
/// asked, so the first frame of a cut is asked early and the frames after it in their turn
/// (preview-throughput D3). A speed changes the stride of a read, so "the same read" is any
/// key of the same size within a few frames.
fn jumps_to(key: &(MediaId, Tick, Quality), before: &[(MediaId, Tick, Quality)], step: i64) -> bool {
    !before.iter().any(|(m, pts, dq)| m == &key.0 && dq == &key.2 && (key.1.0 - pts.0).abs() <= 4 * step)
}

/// One video prefetch pass against the cache: what of the window is MISSING (nearest
/// first, for the decoders), after marking what the window already holds as recent from
/// the furthest frame to the nearest (`BurstCache::refresh`). With that, a frame the
/// playhead reaches sooner outlives one further ahead, and both outlive what was already
/// shown: the LRU alone ranked a prefetched frame by its INSERTION, and the nearest
/// frames of the window, decoded first, were the first to go.
pub fn video_prefetch_pass(
    video: &VideoRenderer,
    cache: &BurstCache,
    t0: Tick,
    q: Quality,
    region_ticks: Option<(i64, i64)>,
) -> Vec<(MediaId, Tick, Quality)> {
    let wants = video_prefetch_wants(video, t0, q, region_ticks, cache.max_video_bytes());
    let key = |&(m, pts, dq): &(MediaId, Tick, Quality)| BurstKey {
        source: source_id(m),
        kind: BurstKind::video(Some(dq)),
        idx: video_cache_idx(pts),
    };
    cache.refresh(wants.iter().rev().map(key));
    wants.into_iter().filter(|w| !cache.contains(key(w))).collect()
}

/// Play `[from, to)` against `cache` with an INFINITELY FAST decoder (diagnostics and
/// tests, video-prefetch-thrash): before every frame one prefetch pass, and every key it
/// misses is filled at once with a blank frame of its decode size; then the frame is read
/// by the plan itself (`frame_plan`, the reads of the preview). No file is decoded, so
/// what the cache does — `unused_evictions` = the performance panel's "dropped before
/// the first read", the video hit rate — is a function of the plan and the budget alone.
pub fn simulate_video_playback(
    video: &VideoRenderer,
    cache: &BurstCache,
    from: Tick,
    to: Tick,
    q: Quality,
) {
    let step = video.grid.frame_step().max(1);
    let mut t = video.grid.align_floor(from);
    while t < to {
        for (m, pts, dq) in video_prefetch_pass(video, cache, t, q, None) {
            let frame = kt_media::VideoFrame {
                pts,
                width: dq.width,
                height: dq.height,
                rgba: vec![0u8; frame_bytes(dq)].into_boxed_slice(),
                has_alpha: false,
            };
            cache.insert_video_span(
                source_id(m),
                Some(dq),
                video_cache_idx(pts),
                step,
                std::sync::Arc::new(frame),
            );
        }
        let _ = video.frame_plan(cache, t, q);
        t = Tick(t.0 + step);
    }
}
