# Proposal: gpu-pass-cost

## Why

The owner, 2026-11-08, on release build kt26.11.08-360b50e6 with his own project (`docs/ktprompts/kkt-26.11.08.txt`,
registry row H9): "стало лучше конечно но все равно я думаю что на моем компьютере всё что могло бы -
работало пошустрее... гораздо..". His journal (`fails/26.11.08/new2/load-report.txt`) shows playback at 25–50 %
of one core of sixteen while the UI waits up to 4.5 s inside one frame and `device_submits` run at ~45 a
second with one or two composites: the integrated Radeon (RADV RAPHAEL_MENDOCINO) is the bottleneck, and
nothing tells what it spends its time on. Found-defects 108: every load number so far ran on llvmpipe; no
pass was ever measured on a real card. On acceleration he decided (docs/orchestrator.md) that all video
math runs as plain WGSL on Vulkan through wgpu, and: "я не собираюсь снимать замеры" — what he gives is a
normal run of the program, so the program must record the numbers itself.

## What Changes

- **The instrument** (`kt_gpu::pass_time`): a GPU timestamp before and after every pass of every plan the
  preview, the offline engine and the export execute, and around every dispatch of the compute host
  (Datamosh's codec, compute generators), where the adapter offers `TIMESTAMP_QUERY` (the preview's eframe
  device and every headless device now ask for it when the adapter has it). The durations are read back
  without a wait (the frame's own readback, or a map the next frame collects), summed per effect and pass
  (`vhs_tape/s5`, `camera_isp/s07`, `composite`, `geometry`, `display`, `datamosh/search`, …) and handed to
  the load watch, which writes one `GPU` record per busy second into the telemetry journal next to its
  `LOAD` record: the adapter, the frame size and the decode size, frames and the device time per frame,
  then each pass's mean and count, the largest first; the same in Diagnostics. Where the adapter has no
  timestamps the record says so and gives the frame's submit-to-done time instead. Nothing is queried
  when nothing composes. `kt-cli preview` prints the same summary.
- **The cost found by mechanism and fixed**: every video effect's pass graph and every shared pass walked
  for passes at full size where a smaller one fits, redundant passes or copies, per-frame resource
  creation, workgroup/shape fit, per-pixel work that is per-row or per-frame, synchronous waits inside a
  frame, uploads of unchanged inputs. The costly ones are fixed within the `FrameTol` tolerances; the
  table (before/after here, what to confirm on his card) is in design.md.
- Guards: no pipeline, texture or buffer created per frame after warm-up, for every effect kind; the pass
  count of each effect's graph pinned.

## Capabilities

- `telemetry`: the GPU record of a busy second.
- `video-plan`: every pass timed; nothing created per frame after warm-up.
- `video-fx`: the costs found and fixed; pass counts pinned.
- `plugins`: a barrier kernel names its passes; a compute step carries a name.

## Impact

kt-gpu (engine, compute host, context, new `pass_time.rs`), kt-plugin-api (`BarrierKernel::pass_names`,
`compute::Step::name`), the plugins that name passes (fx-video-tape, fx-video-camera, fx-video-codec,
generators), kt-app (the eframe device descriptor, load watch, Diagnostics, catalogs), kt-cli (`preview`).
