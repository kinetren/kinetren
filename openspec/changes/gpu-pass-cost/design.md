# Design: gpu-pass-cost

## The evidence and the mechanism

His journal (`fails/26.11.08/new2/load-report.txt`, release kt26.11.08-360b50e6, Ryzen 16 threads,
integrated Radeon RADV RAPHAEL_MENDOCINO, the choice LowPower = that Radeon): during playback the
process holds 25–57 % of ONE core of sixteen; the UI thread is blocked 0.2–4.5 s inside one frame;
`composites` are 1–3 a second while `device_submits` run 35–49 a second. A processor at a quarter of a
core with a UI waiting seconds is a program waiting for the device. Two GPU paths feed it:

1. **The engine** (`kt-gpu/src/engine.rs`): the frame plan as fragment passes — the fused geometry
   pass, every effect's passes (VHS Tape: 14 passes on the 1135×576 tape grid; Camera ISP: 12 passes at
   the frame's size), the composite, the display pass.
2. **The compute host** (`kt-gpu/src/compute.rs`): a SECOND device on the same card, used by Datamosh's
   codec (search, residual, decode per frame) and the compute generators, with a blocking wait per
   job. `device_submits` at ~45/s beside 1–3 composites is this path: the stream stage encodes and
   decodes a Datamosh frame per frame of prefetch.

The class: **work the device does that nobody measured**. Every pass of both paths is a place it can
hide; nothing in the program could say which.

## D1. The instrument: timestamps around every pass, read without a wait

- `kt_gpu::pass_time::Timer`, one per engine: a 1024-query set (512 passes a frame; more run untimed and
  are counted), a resolve buffer, and a pool of map-read buffers. Every `begin_render_pass` of the
  engine (`pass_which`, `pass_geom`, `pass_display`, `clear_tex`, the empty-frame clear) passes
  `timestamp_writes` with the pair of its slot. `end` resolves the frame's queries into a buffer of its
  own in the same encoder; `submitted` asks its map after the submit; `harvest` (at the start of every
  frame, and after the readback of `execute`/`collect`) reads what came back, oldest first, and never
  waits. At most 8 frames wait for their map: past that (no poll came) a frame runs untimed.
- Only `TIMESTAMP_QUERY` is needed, not `…_INSIDE_ENCODERS`: every pass of the engine is its own render
  pass, and pass-level writes are the WebGPU form every Vulkan/DX12/Metal driver offers.
- Every device asks for the feature where the adapter has it: the headless and export devices
  (`context.rs`), the compute host, and the preview's eframe device — `WgpuSetupCreateNew::device_descriptor`
  is wrapped (the descriptor egui-wgpu builds, plus the feature).
- **No timestamps** (the GL backend the live check uses, some drivers): a frame is timed from its
  `queue.submit` to `on_submitted_work_done`, which includes queueing; the record says "from submit to
  done (no timestamps on this device)". t2020 runs both.
- Labels are resolved to names only when summed: a fixed pipe by its name (`composite`, `resample`,
  `layer_mix`, …), the fused geometry pass `geometry`, the display `display`/`glow`, a point-wise
  plugin's own pass by its id, a barrier's pass `id:name` (`BarrierKernel::pass_names`, `p<i>` without),
  a host node inside a barrier's graph after the barrier (`vhs_tape:resample`). The separator is `:`,
  not `/`: the journal reduces any word holding a `/` to a file name (t2021 caught it).
- The compute host: a pair per dispatch (`ComputePassTimestampWrites`), read with the job's own buffers
  after its wait; named by `Step::name` (`datamosh:search`, `generator:noise`).
- One process-wide sum (a mutex taken once per frame and once a second); `take()` swaps it out. The load
  watch takes it every period beside the thread ticks and writes a `GPU` record after the `LOAD` record of
  a busy second; Diagnostics shows the last one; `kt-cli preview` prints the run's.
- **At rest**: nothing composes, so nothing is queried, mapped or summed; `take()` is a lock and a
  `None`. t2022.

What the record cannot say: on a tiling or heavily overlapping GPU the sum of passes may exceed the
span (passes overlap) or fall short of the wall time (the device idles between submits); both numbers
are printed. Compute jobs on the second device overlap the engine's passes on the same card; their time
is printed apart.
