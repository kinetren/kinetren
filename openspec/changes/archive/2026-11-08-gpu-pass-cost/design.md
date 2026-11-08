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

## D2. The class walked: every effect, every shared pass, both paths

The instrument ranked every pass here: llvmpipe 20 (Vulkan, 16 threads, AVX2), 1080p, one layer with
the effect at its defaults over the composite, the minimum of seven frames after two of warm-up (the
machine ran at load 15–20 from other sessions: single readings moved by 30 %, so every before/after
below is an interleaved A/B of the two shader texts, rebuilt alternately). llvmpipe's 128 lanes are
about the arithmetic of his Raphael iGPU (2 RDNA2 CUs); its texture and memory side is not, and it
spills nothing, so the ranking is rough and the card must confirm it (his journal will).

What each mechanism was checked against: passes at full size where a smaller fits; redundant passes or
copies; per-frame creation of textures, buffers, pipelines; workgroup shape on RDNA (wave32/64);
per-pixel work that is per row, per column or per frame; uncoalesced reads; f32 where f16 fits;
synchronous waits and readbacks inside a frame; uploads of unchanged inputs.

| effect / pass | ms a frame here (before) | found | done |
|---|---|---|---|
| Camera ISP (12 passes, frame size) | 115–129 | every helper read re-queried `textureDimensions`, divided by the sensor scale and took two runtime integer modulos per neighbour; raw-front evaluated the per-row and per-column optical-black terms (two hashes each) for each of up to 29 window reads; raw NR the same codes and wraps per neighbour; YUV converted the same 25 neighbours up to 72 times | `begin()` takes the sizes, scale and period once per pixel in every pass; raw-front and raw NR take their window's five columns and rows once (wrap, phase, correction); YUV reads its 5×5 once. **92–97** (raw-front 37→15, raw NR 20→13); t1217 (the GPU graph against the CPU kernel), t1430 and the consequences t1211–t1222 hold |
| VHS Tape (14 passes, 1135×576 grid) | 65–72 | head read: 81 taps × (2 row-kernel + 2 magnetisation reads), each clamp re-querying the texture width; the coefficients are per row; the rest is the simulation itself | `tape_dims()` once per pass (injected at the entry by `shader_texts`): head read −10 %. The 324 reads a sample are the physics — kept |
| Datamosh, compute host | 22 device + 25 host per encode+decode | residual: a thread per 8×8 block with three 64-float private arrays (spills to scratch on a card, 32 640 threads for a 1080p plane); planes uploaded as one u32 per byte | residual: a workgroup per block, a thread per coefficient, the four DCT stages joined by barriers, the basis in the uniform — the same sums in the same order: t1239's pinned bytes hold; 4.8→3.8 ms here, more expected on a card. Planes go up as bytes, four to a word |
| radial blur | 22.8 | zoom: 16 bilinear taps = 64 reads a pixel (its substance); spin: 16 sin/cos per pixel of angles that are per frame | kept; spin's table needs uniform room a barrier pass lacks — listed |
| ripple, lens distort, kaleidoscope, twirl, cylinder3d (fused geometry, warp) | 7–19 | the per-pixel Jacobian probes and the supersampling loop's run-time bounds (masked on llvmpipe, cheap on a card) | kept (render-speed-no-card D4 already specializes the shapes without probes) |
| sharpen, drop shadow, glow, blur | 4–6 | mip down/up + one pass; the mip runs at 1/r of the frame already | kept |
| edges, chromatic aberration, halftone, vhs (imitation), pixelate | 3–5 | one pass each, a few reads a pixel | kept |
| color grade, duotone, posterize, vignette, noise, greenscreen, mirror | 2–3 | one pass each, point-wise | kept |
| corner pin, pan3d, event grid (fused geometry) | 3–6 | one fused pass | kept |
| composite | 1.1 a layer | one full-frame pass a layer, ping-pong (determinism, t183) | kept |
| display + glow target | 0.4 at 640×360 | one pass to the egui texture + 32×18 | kept |
| resample (decode scale, Mix fit) | 1–1.5 | bilinear, 4 reads | kept |
| readback (`execute`) | — | a `poll(Wait)` — the readback path's purpose (offline, export ring waits only for its oldest) | kept; the preview's `present` never waits |
| uploads | — | `UploadCache` keyed by the content serial: an unchanged frame is never uploaded again | kept |
| per-frame creation | — | none after warm-up, every effect and preset and the heavy stack (t2023); the preview memo keeps each new composite until its 512 MB are full, then recycles (cached-range-plays, by design); the timer's own buffers stopped growing once a non-blocking poll collects the preview's maps | guarded by t2023, t2024 |
| per-pass bind group + uniform `write_buffer` | CPU, ≈ µs a pass | made per pass per frame on the composing thread | listed (CPU, not the card; the UI thread is `ui-never-waits`'s) |
| f16 | — | the textures are already 8-bit integers or 32-bit words; `shader-f16` is not on every device (GL) | rejected |
| workgroups (compute) | — | search/decode 8×8 = 64 (one wave64 or two wave32), generators 16×16, residual now 64 a block | kept |

Not a cost of the passes but found by the instrument: llvmpipe compiles a pipeline variant at its first
draw, inside the frame (a 2-frame preview showed `glow:resample` at 2 s). A card compiles at
`create_render_pipeline`; ours builds every fixed and plugin pipe at engine creation, only the fused
geometry pass's specialized shapes (`geom_specialized`) compile at the first frame of a new shape.

## D3. What his card must confirm

His next normal run writes `GPU` records into `telemetry-journal.log`/`load-report.txt`. They answer,
with no measurement by hand: (1) is the Radeon the bottleneck — device ms a frame against 40 ms (25 fps);
(2) which passes — expected Camera ISP and VHS Tape first; (3) the Datamosh compute jobs a second and
their device time beside the preview's; (4) whether the RADV residual restructure paid more than here.
If `ms a frame` is far below 40 while the UI still waits, the wait is not the passes (queueing, the
compute device's jobs, `ui-never-waits`).
