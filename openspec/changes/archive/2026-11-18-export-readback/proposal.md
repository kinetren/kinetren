# Proposal: export-readback

## Why

Two items of the owner's list `kkt-26.11.18` (after stage 19):

- **T3.** "отрендерил даже самый простой регион - и скорость 1x cur - это пиздец." His trace
  (`fails/26.11.18/render_simple_region`, kt26.11.17, Ryzen 7700X iGPU, RADV, `h264_vaapi`): 2238 frames
  of 1080p60, 37.29 s of timeline in 39.61 s — 0.94× real time. The compose thread spent 94.5 % of the wall
  in the readback, 82.2 % waiting for the device. The device itself worked 1.06 ms a frame (p50 of its
  `GpuFrame` records, its pass timestamps), yet a frame came back 52.7 ms after its submission (p50; p10
  43.7 ms). The export's readback ring holds a fixed three frames, so its rate is three frames per
  submission-to-done latency: 3 / 52.7 ms ≈ 57 frames a second, the 0.94× of a 60 fps project. The render
  is bound by how long the device takes to hand a frame back, not by any work.
- **T4.** "объедини лог рендера в телеметрию, нахуя ты его выделил отдельно то?!?" Every line of
  `render.log` is already in the telemetry journal and the structured trace (`EXPORT` journal lines, seen in
  his trace at +70.9 … +111 s): the file is a duplicate he has to collect separately.

## What Changes

- The export's readback ring is as deep as its bytes allow (a budget over the frame's staging size, at least
  three), and the compose thread hands on every frame the device has finished as soon as it has finished,
  waiting only when the ring is full. The depth in use then follows the device's own latency: a device that
  takes 50 ms to hand back a 1 ms frame keeps a dozen in flight, a software one keeps one or two. The bytes
  written do not change (invariant 11); the export stays in-process (invariant 9).
- The trace records how deep the ring was and how long a frame took from submission to collection, and
  `kt-cli trace … export` states it beside the device's own time a frame, for older traces too (from their
  `GpuFrame` and `export_frame` records); the export view lists the run's `EXPORT` journal lines.
- `render.log` and `render.log.1` are no longer written. The render's lines go where every other record
  goes: the window, the telemetry journal and the trace. The window no longer shows a log path; after a
  failure "Open log" opens the telemetry journal.

## Capabilities

### Modified Capabilities
- `export`: the readback ring's depth and hand-on; the render log lives in the telemetry, not a file.
- `telemetry`: the trace's export view states the ring, the latency and the device time a frame, and the
  render's journal lines.
- `durability`: the render log is no longer among the data directory's files.

## Impact

kt-gpu (`engine.rs` ring, `pass_time.rs` pending frames), kt-engine (`video_plan.rs` `VideoGpuExec`,
`export_pipeline.rs` compose loop and profile), kt-time (`trace::export_stage`), kt-cli (`trace_query.rs`),
kt-app (`render_dialog.rs`, i18n catalogs, tests, live `render`). No format change; OTIO unaffected.
