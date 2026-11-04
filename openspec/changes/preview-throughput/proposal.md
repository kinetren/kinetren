# Proposal

## Why

The owner (2026-11-03): "первым меня интересует производительность превью рендера и финального
рендера и другие проблемы производительности" (first of all I care about the performance of the
preview render and the final render, and other performance problems). The final render was rebuilt
as a pipeline (`export-throughput`); the preview was not.

Measured on the showcase edit with a new headless bench (`kt-cli preview`, the real player against
a wall-clock output device, 34–46 s, release, footage in /dev/shm). This machine has NO real
graphics card: "GPU" below is llvmpipe on 32 cores.

- At full quality (1080p) the preview showed 9.5–12.8 of 25 frames per second, with 267–292
  incomplete composites and 42,762–82,482 frames of audio underrun.
- The decoders produced 3,844–5,413 pictures to deliver about 550: the sharp layer and its blurred
  backdrop ask one file at two decode sizes, `MediaWorker` serves each size as a span of its own,
  and the second span, behind the cursor, seeks back and rebuilds the decoder (55–75 reopens). The
  export fixed this for itself (D7, D9); the preview did not get it.
- Once decoding keeps up, the UI thread is the limit: about 33 ms per frame at 1080p spent uploading,
  waiting for the device and copying the frame back out, inside a 40 ms frame, before the frame is
  converted and uploaded to the device a second time. The CPU path composes on the UI thread, one
  frame at a time: p95 381 ms.

Software decode is not the limit, so hardware decode is not part of this change.

## What Changes

- **`MediaWorker` decodes a moment once.** A span carries every decode size of a moment and goes
  through `read_video_span_qualities`, which also gives a file read at two places its second
  decoder. The same source stays owned by the same worker thread (invariant 5).
- **The GPU preview shows the composite without reading it back.** The frame goes from the
  composite to a display texture the UI draws, so the UI thread neither waits for the device nor
  copies the frame out and in again. Readback stays for `execute_into` (export, tests, fallback).
- **The CPU path composes ahead on worker threads**, so a frame is ready when the display reaches
  it and the UI thread only picks it up. The UI's synchronous compose stays for a miss.
- **`kt-cli preview`** prints V-fps and V-skipped as the performance panel counts them, and where a
  frame's time went, stage by stage.
- **The first frame of a cut is asked before the window reaches it**: a decoder needs the same time
  to reach it however near it is asked.

## Capabilities

### Modified Capabilities

- `media-index`: the worker's span covers every decode size of a moment.
- `video-plan`: the preview's two executors keep up without standing on the UI thread.

## Impact

`src/libs/kt-engine` (worker.rs, player.rs, preview_bench.rs, video_plan.rs), `src/libs/kt-gpu`
(engine.rs, pool.rs, a display pass), `src/apps/kt-app` (`draw_preview`, the panel), `src/apps/kt-cli`.
Invariants 5, 6, 7, 11, 12 hold: no live source gains an owner, the audio clock is untouched, the
plan and its kernels are the same, offline paths are untouched, nothing allocates per frame in draw.
