# Design: render-speed

kkt-26.11.15 N3 (line 6): "п.3. финальный рендер у нас очень медленный во всех версиях, медленее чем превью...".

## D0. What was measured

Release build of `90382a9a`, this machine (i9-13900KF, 32 threads, no GPU: llvmpipe), footage and projects in
`/dev/shm`, the showcase from `/media/e/developer/kt.9/sources/showcase-media` (`kt-cli showcase … [--heavy]`,
104 s at 1080p25). From about 13:15 a sibling's kt-app test run held the load at 30–80, so every A/B after
that was interleaved and repeated; the numbers below carry their load where it mattered.

| what | wall | note |
|---|---|---|
| plain, whole project, export (4 llvmpipe devices, the default) | 107.2 s | load ~1; compose 96 % busy on each of 4 workers, device wait 361 s summed |
| plain, whole, 4 devices / 1 device, interleaved twice | 131.8, 129.2 / 144.4, 171.9 s | load ~40 |
| plain, 20–40 s, preview (`kt-cli preview`, 1080p) | 20.1 s | V-fps 24.0, 18.5 ms of device time a frame |
| plain, 20–40 s, export 4 devices / 1 device | 31.8 / 12.5 s | load ~1; 4 devices of 8 threads each 17.7 s, 2×16 13.0 s |
| heavy, 20–30 s, export 1 / 2 / 4 devices at 1080p | 128.8 / 145.0 / 126.1 s | the device bounds: 2.2 s of llvmpipe time a frame |
| heavy, 20–30 s, preview at 1080p | — | 2 of 250 frames shown (V-fps 0.2) |
| heavy, 20–30 s, export / preview at 640×360 | 50.9 s / 36 of 250 frames in 10 s | |

- **The export is not slower than the preview at the same size here.** On the plain piece the preview needs
  18.5 ms of device time a frame and plays in real time; the export does the same piece 1.6× faster than real
  time on one device. On the heavy piece the preview cannot play at all and the export finishes.
- **Inside the single-device export the frame runs in `queue.submit`.** The new submit stages (D2) show 12.6 of
  13.05 s of the compose thread's "encode" inside `queue.submit`: llvmpipe executes a submission on the calling
  thread. One device is one frame at a time; several devices are frames side by side, but each starts a thread
  per core. That is why the count wins on some stretches and loses on others (D1).
- **The owner's preview is not the same size as his render.** His `rendereg_2` load report: the preview's
  frame 640×360 (the preview fraction, `preview_quality_for`), VHS Tape 21.6 ms a frame on his AMD iGPU. A
  1080p render draws nine times those pixels. Effects with sizes of their own do not scale with pixels (here the
  heavy piece at 640×360 took 40 % of its 1080p time), so the factor on his machine is between those two and
  is for his run to say.
- **The GUI's ring.** On his iGPU the export shares the display's ring and is admitted last, with the slack
  the display's present leaves (loop-laps-regression D2); a pass longer than a piece is admitted only on an
  idle ring. On llvmpipe each device is its own ring, so this cannot be measured here (found-defects 692): the
  `gpu-share` A/B on one llvmpipe device gave the offline work 37 frames/s with the arbiter, 25 without.

## D1. The device count of a software adapter stays (rejected change)

Tried: one export device for a software adapter (a device already spreads each pass over every core; four
devices of 32 threads oversubscribe). The light piece went from 31.8 s to 12.5 s, but the whole project went
from ~130 s to 144–172 s, the heavy piece did not move, and the isolated kernel bench (`bench_engines_in_parallel`)
gave four devices of 32 threads +25 % on VHS Tape. Four devices of 8 threads lost everywhere. No rule both
ways was found; the change was reverted and the finding is found-defects 710 (a rule per stretch, by the device
time the pass timer measured, is the candidate).

Also rejected: composing on the iGPU and the CPU's llvmpipe at once on the owner's machine. Which frame a device
composes would have to be fixed by a rule, not by who is free (invariant 11: video byte for byte between runs),
and an llvmpipe frame of his project costs ten times the iGPU's while it takes the cores decode needs.

## D2. Every export writes where its time went

`export_mp4_with_backends` keeps an `ExportProfile` whether or not the caller asked (`ExportOptions::profile`
stays the way to read it live). When the export ends it writes:
- an `Ev::ExportStage` record per stage (`kt_time::trace::export_stage`: `aux` the stage, `a` busy ns summed over
  the stage's threads, `b` the wall, `c` frames, `id` frame workers) and a `WALL` record (`a` wall, `b` the
  timeline ns covered);
- `ExportStats::stages`, one line (`ExportProfile::summary`): frames, output size, real-time factor, form, and the
  busy stages largest first. The render window writes it into the telemetry journal as a NOTE, so the load report
  the owner sends carries it.
The clock is read only into this record (invariant 11 holds: nothing steers by it). The single-device path
(`GpuFxEngine::submit`/`collect`) now fills `last_stages` — upload, encode (with `submit_ns`), device wait, copy
out — as `execute_once` did for the per-worker devices; before, those lines read zero for the one form a card
gets.

## D3. `kt-cli trace DIR export`

For each export of a run: frames, timeline against wall, the form, each stage busy at all with its share of the
wall, and from the device rings inside the export's window: its `offline` admissions and their waits, the pieces
cut, and whether a display presented on the ring (`GpuAdmit.c == 1`). That last line is what answers whether the
ring costs his render anything.

## Class walk: every path that renders offline

| path | goes through | writes its stages |
|---|---|---|
| the render window's export, whole project or region | `export_mp4_with_backends` (range = region) | yes, trace + journal |
| `kt-cli export` (`--range`, `--size`, `--serial`) | the same | yes, trace + stderr |
| the serial export (`opts.serial`) | `render_raw_video_profiled` + `render_raw_audio_range` | yes (`id` 0) |
| the pipelined export, one device (a card) | `export_pipeline::run`, compose thread submit/collect | yes; its device stages are new |
| the pipelined export, a device per worker (software) | the same, `execute_once` per worker | yes |
| the stream (Datamosh) export of stream-round-trips | the pipeline's `Job::Planned` | yes (plan time on the compose thread) |
| the hw-encode cascade | the muxer under the writer: the `encode` stage | yes |
| `render_pipelined` / `render_pipelined_budget` (tests' sink) | `export_pipeline::run` without the profile | no: no person waits on them |
| `render_raw_video_range_with` (`kt-cli render`, golden) | the serial loop without a profile | no: a test tool |
| reel strips, posters, effect thumbnails, the loop fill | `side_engine`, per job | no: not an export; their jobs are `Job` spans (`trace slow`) |

## Callers of what moved

- `ExportStats` gained `stages`: built in `export.rs` (two places), `export_pipeline.rs` (`..Default`),
  `layout_tests.rs` t957's literal (now `..Default`); read by `render_dialog.rs::on_result` and kt-cli.
- `ExportProfile` is now always created: `opts.profile` callers (kt-cli `--time`, tests) see the same numbers.
- `GpuFxEngine::submit`/`collect` reset and fill `last_stages`: callers are the export's compose thread and the
  tests `pass_time.rs`, `local_cost.rs` (they do not read stages); the preview reads `last_stages` after
  `present`/`execute` on its own engines, never after a submit.
- `Ev::ALL` grew to 53; `trace_query` knows the kind by `is(r, Ev::ExportStage)`; older readers skip an unknown kind.
- `kt-cli gpu-share --mosh none` skips the stream check only; every other mode is as before.

## Files touched (for the merge beside `cpu-idle-work` and `render-settings`)

kt-time `trace.rs`; kt-engine `export.rs` (ExportStats, the profile at the top of the video phase and the trace
before Done — not `MuxConfig`), `export_pipeline.rs`; kt-gpu `engine.rs` (submit/collect only); kt-app
`render_dialog.rs::on_result`, `layout_tests.rs`; kt-cli `main.rs` (the `export` arm: `--range`, `--size`, the
stages line), `trace_query.rs`, `trace_query/tests.rs`, `gpushare.rs`. No worker pool was changed.
