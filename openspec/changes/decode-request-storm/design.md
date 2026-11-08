# Design: decode-request-storm

## The mechanism

A PRODUCER of decode requests that asks more than the cache keeps until its CONSUMER reads it, so the
decoder works for the eviction:

1. A stream stage (Datamosh) ordered the WHOLE history of its frame at once: with the mosh gate on every
   I-frame after the stream's start is dropped, so the frame depends on everything since the event's start
   (up to 600 positions). At 1080 lines a position is an 8 MB frame; 400 of them are 3.3 GB against the
   preview's 512 MB cache.
2. The decoder read that order front to back, forward of its cursor, as it should. But the stage read the
   frames only when a COMPOSITE came (4 a second on a pause in the app), and a composite computes the
   stage from the earliest missing position. LRU evicted the oldest inserts — the front of the history,
   exactly what the stage needed next. Every decoded frame was dropped unread (the journal's "prefetch
   thrash: N" with N equal to `decoded`).
3. Each composite then re-submitted all ~392 still-missing positions (4 × 392 = the journal's 1568 a second;
   the counter counted repeats of keys already queued). The decoder's cursor now stood past them, so it went
   BACKWARD: `pick_next` took the NEAREST key behind the cursor, and a span tops up only forward of its first
   key, where everything was done — one key per span, a seek and a decode from the keyframe per frame,
   walking the history from its end to its start (the trace: spans `n 1` at 5847000, 5845500, 5844000, …).
   By the time it reached the front, the back was evicted. A circle, for as long as he looked.
4. On a loop the same, at the rate of the display: ~430 positions per composite, 46 composites a second.

And the UI side of the same producer: when the history WAS all cached, the composite computed all of it on
the UI thread — 8.97 s for one composite on his relinked project at 480 lines (the "UI thread was blocked
for 6272 ms inside one frame" lines of his journal sit right there).

Reproduced on his relinked project (release, footage in /dev/shm, `kt-cli pause-scan --from/--to`): event
293 with its Datamosh at 1080 lines, paused at 2706.2 s: NEVER complete in 30 s, 282 composites, 88 653
requests, 2 026 frames decoded (67 a second) — his journal's signature. The model test t2044 replays it:
shown 0, 467 dropped unread, 5 010 pictures for a 103-frame history, 231 reopens.

## What was done

- **D1 The order is a window of the history** (`StreamPacing::order_bytes`, a quarter of the video cache):
  `layers_at` orders the EARLIEST positions the stage lacks, as many as fit (at least one). The kernel
  already returns them ascending, both sides interleaved, for the export's pre-roll. The order stays a
  prefix of what the stage reads, in the order it reads it (invariant 4).
- **D2 The prefetch thread advances the stage** (`VideoRenderer::stream_advance`, now returning "still
  pending"): at the frame on screen, and while playing ahead of it — wrapping in the loop region — as many
  frames as the display reaches before the next pass, twice over, at least 4, at most what a quarter of the
  memo keeps at two frames per position. A pass that left a stage pending runs again on a pause (the pause
  skip applies only when nothing is pending, so the program at rest still costs nothing). Advancing at the
  composite rate was the coupling that broke; the decoder fills the window, this thread consumes it, the
  next window is ordered by the composite and the look-ahead.
- **D3 The composite never computes a history** (`compose_steps` = 2): a stage whose frame lacks more is
  reported incomplete on a pause (the frame itself is waited for, while the prefetch computes it), and while
  playing shows the latest frame the stage holds on that frame's stream (`StreamKernel::held`, a memo lookup
  only — a late frame, as a late decode shows its last one, `StreamPacing::hold`). Without it, a stage slower
  than real time never met the playhead and the picture stood (480 lines on llvmpipe: V-fps 0.3); the old
  code got 0.8–4.3 there by blocking the UI 0.2–2.9 s per composite. A composite that showed a held frame is
  not recorded as the moment's kept composite (`stream_held`), so the bars do not claim a moment plays
  without work while its own frame was never composed.
- **D4 The worker** (`worker.rs`): behind the cursor the pick is the START of the run of queued keys that
  ends at the nearest one (same priority, ≤ 64 moments, ≤ 2 s back, the same sizedness), so one span reads
  the run forward; a key already queued, or in the span being decoded (`Queue::in_flight`), is not queued
  again and is not counted in `decode_requests` — that counter now means work entering the queue.
- **D5 Offline paths with their own sources**: the reel strip and the project poster read the whole
  history into a 16–64 MB cache in one go, losing its front before the composite. `compose_offline_frame`
  runs the export's `stream_preroll` (now public, its batch half the budget of the cache it is GIVEN; the
  export's cache is `EXPORT_VIDEO_CACHE_BYTES`, so the export's batches are the same number as before).

Rejected: making the cache or the memo bigger (the history at 1080 lines is 3.3 GB; a bigger cache moves the
cliff, it does not remove the coupling); ordering the history in the worker by priority "earliest first"
(the worker does not know that a stream's keys are a sequence its consumer reads in order — the producer
does, so the producer bounds it); running the stage on the worker thread (invariant 5 keeps the decoder's
thread a decoder's, and the stage needs the compute device).

## Measurements

Release, this machine (llvmpipe, 32 threads; other sessions' builds pushed the load to 10–50, so pairs ran
back to back). His relinked project, event 293 (Datamosh, gop 15, mosh 1, 480 lines unless said), preview
960×540 (pause-scan 960×540 / 640×360), footage in /dev/shm. `before` = 7c6ffe06.

| scenario | before | after |
|---|---|---|
| paused at 2706.2, 1080 lines: complete? | never in 30 s | the decoder keeps up; the encoder at 1080 lines on llvmpipe (~8 positions/s) is the limit |
| — requests / decoded in 30 s | 88 653 / 2 026 | 389 / 257, nothing dropped unread |
| paused at 2706.2, 480 lines: time to the frame / longest composite | 9.1 s / 8 970 ms | 6.0 s / 111 ms |
| 0.22 s loop, cold, 1080 lines: pictures / reopens / dropped unread | 19 378 / 162 / 770 | 65 / 1 / 0 |
| 0.22 s loop, cold, 480 lines: V-fps / longest composite | 11.6 / 5 484 ms | 35.9 / 52 ms (first build of D3) |
| 1.2 s loop, warm, 480 lines: V-fps / longest composite | 28.7 / 807 ms | 28.8 / 73 ms |
| linear 2700–2710, 480 lines: V-fps / composite p50 | 0.8 / 866 ms (4.3 / 206 ms at low load) | 10.8 / 27 ms |
| linear 2700–2710, 144 lines: V-fps / composite p50 | 9.1 / 54 ms | 7.0 / 27 ms |
| plain showcase 10–20 s, 40–50 s (no stream): V-fps | 24.5, 11.2 | 25.0, 23.1 |

At 1080 lines the picture of a cold loop still waits here: the Datamosh encoder itself runs ~8 positions a
second on llvmpipe (the decoder is idle, the UI is free). On his Radeon that cost is `gpu-pass-cost`'s to
measure; this change takes the decoder and the UI thread out of it.

## The class, walked (the owner's rule)

The mechanism: a producer that orders more than the cache keeps until its consumer reads, or re-asks what is
already in flight, or a cancellation that throws away what will be read. Every producer of decode requests:

- **The playback composite** (`compose_preview_measured`, Playback): orders `layers_at` of its frame each
  composite and withdraws the Playback keys not in it. Found: re-asks counted as requests (D4); for a stream
  layer the whole history (D1). Its withdrawal keeps the window (the window IS its layers). Fixed.
- **The near look-ahead** (`video_prefetch_pass`, NearPrefetch, `video_prefetch_wants`): already bounded
  by half the budget at whole frames (video-prefetch-thrash); for a stream layer each window frame named the
  whole history, now its window (D1). Its withdrawal keeps what the window asks. Fixed through D1.
- **The cut leads** (`jumps_to`, `CUT_LEAD_KEYS`): `layers_at` at a cut's first frame — for a stream clip its
  window (D1); for footage one key a cut. Checked.
- **The stream stage's needs and pre-roll**: the preview (D1–D3); the export's serial pre-roll was already
  windowed (batches of half its cache) — unchanged bytes; the reel strip and poster were not (D5). Fixed.
- **Nested reels**: `layers_at`, `stream_advance`, `stream_frame_bytes` and the pacing descend into the
  sub-renderers; a stream slot on a reel instance is skipped (datamosh-codec-path). Checked.
- **Morpher layers**: they reach the plan as effective clips; the same path. Checked.
- **Blended frame pairs**: both pts are one `clip_frame_keys` call, ordered together; a stream layer does
  not blend (the stage replaces the source). Checked.
- **Audio bursts** (`prefetch_audio_pass`, the render thread's Playback window): forward windows, deduped in
  the queue; the dedup now also does not count repeats (D4). Checked.
- **Thumbnails** (`thumbs.rs`): their own short-lived sources on their own thread, one frame per key, no
  stream (raw source frames). Checked.
- **FX thumbnails** (`fx_thumbs.rs`): a picker preview of a still sample through the chain; no stream
  history ordered. Checked.
- **Reel previews and posters** (`reel_preview.rs`): D5. Fixed.
- **The bin scrub** (`bin_panel.rs`): goes through the thumbnail path. Checked.
- **`request_video`** (diagnostics): one Playback key; a caller that waits re-submits — now a no-op while
  queued or in flight (D4).
- **Export** (`export_pipeline`): own sources, chunks hold their frames (export-pipeline-determinism);
  a stream project keeps the serial export with its windowed pre-roll. Checked.

Every cancellation path:

- **Generation** (`cancel_generations_below`, a seek): drops queued keys of old generations; the new
  composite and look-ahead order the window again. An in-flight key is not interrupted (unchanged); its
  resubmit at the new generation is dropped as in flight — it lands in the cache anyway. Checked.
- **Withdrawal** (Playback by the composite, NearPrefetch by the look-ahead): with D1 both producers ask the
  same window for a stream layer, so neither withdraws what the stage reads next. Checked.
- **Eviction** (LRU): the window is a quarter of the budget, consumed by the advance within a pass or two;
  t2044 holds "no decoded frame dropped unread" with a 103-frame history in a 4 MB cache. Fixed.
- **The span's tail** returned on more urgent audio: unchanged (it re-enters the queue, not counted).
- **The negative cache** (`failed_video`): unchanged.

Situations: linear play (D2 ahead), loops short and long (D2 wraps in the region; the measured loops),
a seek (generation; the window re-ordered), a paused preview (D2 keeps passing while pending; D3 waits for
the frame), after an edit (the memo's keys change with the rows; the history is ordered window by window).

## Found, not fixed

- At 1080 lines the stage's MEMO (256 MB) keeps ~20 positions (a clean entry is three planes): a loop longer
  than that recomputes its history every lap — each lap's start was evicted by the lap's end. The kept
  composites do not help: a stream layer must render to be replayed. A bound on the frames a loop needs, or
  a memo that keeps the region's start, is the next step (`loop-stream-memo`).
- The prefetch pass's look-ahead walk calls `needs` for every frame of a 2 s window, a chain walk each:
  160–280 ms a pass on his relinked project. Cheaper needs (or one walk per stream per pass) would let the
  passes run at their 100 ms.
