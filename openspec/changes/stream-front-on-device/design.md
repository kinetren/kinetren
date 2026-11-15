# Design: stream-front-on-device

## Context

found-defects 543. stream-input-order (row 484) made a stream stage read the output of the slots in
front of it: `ClipStream::with_frame` runs them through an injected executor, the offline engine's
sibling, and hands the kernel bytes; Datamosh's walk then uploads those bytes to the compute host
(`Batch::convert` → `Init::Bytes`). Per history position at 1080 lines that is a composite on one
device, a readback (submit, wait, map, copy out), and an upload of 8 MB to another device: ≈ 16 ms
of the advancing thread warm (stream-input-order, "Cost").

Mechanism (the class): **a GPU result read back to the processor only to be uploaded to a device
again.** The bytes are never looked at in between; the trip exists because producer and consumer
sit on different devices or the API between them speaks only bytes.

## Decisions

### D1. The front runs on the compute host's device

kt-gpu's compute host is a device of its own (Vulkan/DX12/Metal whatever the interface uses,
compute-kernels). The front's engine is now a `GpuFxEngine` built on THAT device
(`kt_gpu::front_on_compute`: `compute::host_device()` → `ComputeDevice::context()`), still an engine
of its own (its own lock, arena and pools), so a walk's front waits on no composite. Both submit to
the one queue of the device, so the arbiter sees them on one ring (`RingKey` of the same device).
`compute::open` records the device it opened (`OPENED`); `host_device` answers only when the
process's compute host is that device — another host (the gpu-share bench's shared device) or none
leaves the front on the offline sibling with bytes, as before.

Rejected: the compute host on the offline device. The offline device may be GL (`WGPU_BACKEND=gl`,
the live check), which has no compute; and the offline device is the preview's slow-chain route,
which a codec walk would then queue behind.

### D2. A held frame: one buffer, two crates that never see wgpu

kt-plugin-api gets `compute::Held { len, inner: Arc<dyn Any> }` and `Init::Held(&Held)`: a job buffer
that starts as a copy, on the device, of a buffer the host holds. The host downcasts `inner` to its
own type (`held::HeldBuf`: the buffer and its device) and refuses a buffer of another device before
anything is encoded (an `Err`, never a panic; the codec would then `compute::fail`, so the engine only
returns `Held` from an engine on the compute device — `hold_for_compute`). Dropping the last clone
gives the buffer back to the device's spares.

`VideoGpuExec::execute_held` (default `Unsupported`) is `execute_once`'s resolve and passes in the
SETTLED form (the same bytes, gpu-leftovers D3), then a copy of the result texture into a buffer of
tightly packed RGBA — a width whose rows are not a multiple of the copy alignment goes through a
padded buffer and a row-by-row device copy — submitted and NOT waited for: the codec's job, submitted
later on the same queue, is ordered after it. An incomplete plan is `Incomplete`
(no-partial-picture: nothing kept). An empty plan answers `Unsupported` (nothing to keep; the bytes
path gives the transparent frame).

### D3. The stream input says where a frame lies

`StreamInput::on_device(side, k)` → `OnDevice::{Held, Missing, Host}` (default `Host`). kt-engine's
`ClipStream` answers `Held` for a frame with slots in front of it, `Host` for a decoded frame with
none (that upload is the decode's, no round trip), `Missing` when the frame or the front's executor
is missing (stream-input-order D5: never the decoded picture). Datamosh's walk (`walk.rs` `source`)
asks it first and converts a held frame with the same `datamosh:convert` step on the device
(`Batch::convert_held`); the reference walk, the pass-through and the CPU encoder keep `with_frame`.

### D4. The front memo keeps both forms

The memo keys by (stream address, form): a held buffer and read-back bytes are separate entries, so
a reader of bytes (the reference walk) never gets a buffer and the other way round; both count
against the same 64 MB (row 542 stays: the memo is outside the ledger). The held buffers' spares
(at most a codec batch's bytes, 192 MB) report to the ledger as compute buffers, so a walk's batches
after the first create nothing.

### D5. The A/B and the instruments

`KT_FRONT_HELD=off` (process) and `with_front_bytes` (a thread, tests) run the bytes form;
`kt-cli stream-cost` prints how many front runs stayed on the device; `compute::held_reads` counts
job buffers that started from a held frame.

### D6. Row 561: a ready geometry shape moves the forms' revision

`Compiled::shape` answers `None` (the interpreter) while a shape compiles, then the specialized pass.
t1766 holds the two equal on the adapter it runs on, but nothing proves it on a card, and the memo
keys (`plan_key` mixes `forms_revision`) did not change when the form did — the mechanism of row
510. `compile_shape` now calls `forms_moved()` when a specialized pass becomes ready. Cost: each
shape's readiness drops the composites kept under the old revision once (a composite whose resolve
saw the revision move is not kept); a shape becomes ready once per device.

### D7. Row 562: the calibration's result apart from its lock

The calibration's bracket moved out of the `calib` lock into `best`, read with a short lock: a job's
records no longer wait behind another job's calibration (the old `to_trace_ns` took `calib.lock()`,
which a calibration holds through its ring admission and its waits). `calibrate` says whether it was
skipped (`try_lock` found another calibrating); a skipped job whose calibration is older than
`CALIBRATION_AGE` places no record (`aux` = u32::MAX − 1, `b = c = 0`) and counts it
(`stale_records`). A job that is not skipped either calibrated or found a fresh one.

## Class walk — every GPU result read back and uploaded again

| # | Path | Readback | Re-upload | Devices | How often | Verdict |
|---|---|---|---|---|---|---|
| 0 | Front of a stream stage → codec | `video_stream.rs` `run_slots` `execute_once` | `walk.rs` `source` → `Batch::convert` `Init::Bytes` | offline sibling → compute | per history position | **fixed** (D1–D3) |
| 1a | Codec output → composite | `walk.rs` `flush` reads planes; `fx-video-codec` `lib.rs` `to_rgba` | `video_stream::run` `fit_into` (CPU) → a layer → `UploadCache` | compute → display / offline / export | per frame shown | avoidable: a held LAYER source in the plan (the executor would read a compute buffer as a texture); found-defects 620 |
| 1b | Codec walk state between batches | `walk.rs` `flush` (recon, residual, checkpoints into `StreamMemo`) | `walk.rs` `dev` `upload_planes`, `upload_packet` | compute → compute | per batch | avoidable for state the next batch reads; the memo's checkpoints need host bytes only to outlive device buffers; found-defects 621 |
| 2 | Preview slow chain (`SLOW_CHAIN`) | `player.rs` `execute_into` on the offline engine | `keep` → display `PresentMemo` (`write_texture`); and the egui texture of the preview | offline → display | per slow frame | required across devices (the offline device is another device by design, scheduler-followups D6); the second upload into egui is a found-defects 622 question (unverified that egui shows that texture) |
| 3 | Loop fill | `loop_fill.rs` `compose_fill` | `keep` → display memo | side → display | once per unshown loop moment | required while the side engine is another device (its point: out of the display's queue) |
| 4 | Reel strips, posters, effect cells | `reel_preview.rs` `compose_on`; `fx_thumbs.rs` `execute_into` | CPU `fit_into` → egui `load_texture`; the strip also to disk | side → UI | once per job (cached) | required: the UI texture is another device and a disk tier |
| 5 | Export (serial, pipeline) | `export.rs`, `export_pipeline.rs` `execute_once` / `collect` | software encoder: none; VAAPI: `av_hwframe_transfer_data` | export → CPU → VAAPI | per frame | required: a CPU encoder or the file; wgpu↔VAAPI interop does not exist |
| 6 | Generators drawn by a compute shader | `kt-plugin-api` `generator.rs` `host.run(reads)` | the frame as a layer → `UploadCache` | compute → display / offline / export | per frame of an animated kind | avoidable with the same held layer source as 1a; found-defects 620 |
| 7 | Analysis | — | — | — | — | none: `kt-app` `analysis.rs` is audio only |
| 8 | Memos | `ReadbackCache`, the front memo, `StreamMemo` | through 0, 1b, 2, 3 | — | — | the stores behind those rows, no trip of their own |
| 9 | Nested reels | — | — | same device | — | none: a sub-composite is a `ChainSrc::Nest` on the device |

Timestamps (`pass_time.rs`, compute stamps) are read back, never uploaded: not this class.

## Callers walk

What the change moves, grep'd across `src/`:

- `install_stream_front` / `stream_front_engine`: kt-app `plugins.rs`, kt-cli `main.rs`, testkit — all
  take the compute-device engine now; their comments said "offline device" and were corrected.
- `prewarm_stream_front` (player): warms `front_exec()`, now the compute-device engine — the device the
  front runs on, so the prewarm still reaches it.
- `park_offline_engine` (kt-app exit): parks `FRONT` too; it now waits on the compute device, which
  the compute host's own exit guard also covers.
- `compute::install` callers: kt-app, kt-cli (`streamcost::open`, which wraps `kt_gpu::compute::open` —
  `OPENED` is set, the held path works through the counting wrapper), testkit, kt-media's generator
  tests, kt-app's idle and thumb tests (`kt_gpu::compute::open`). `gpushare` installs a host on a
  shared engine device (`ComputeDevice::on_context`): `OPENED` stays empty, the front keeps bytes.
- `Init` matchers: the compute host (handled), `Batch::run` (handled), `streamcost` and the codec's
  `packed.rs` count uploaded bytes from `Init::Bytes` only — a held frame is no upload, which is what
  they should report.
- `VideoGpuExec` implementors (kt-gpu engine, test stubs in kt-app and export.rs): the new method has a
  default (`Unsupported`), so every stub keeps the bytes form.
- `with_frame` readers: Datamosh's walk now asks `on_device` first; `mosh::render_reference`,
  `lib.rs` pass-through and `mosh.rs` CPU encode still read bytes (the reference and no-device paths).
- `forms_revision` consumers: `plan_key`, the readback memo's keep check, the present memo — all move
  with a shape's readiness now.
- `to_trace_ns` callers: the `gpu_start` records only.

## Cost

(measured on release — filled below by task 3.2)

## Risks

- A machine whose compute device opens but on which no engine builds (a backend that compiles the
  compute shaders but not the composite's) keeps the bytes form: `front_on_compute` returns `None`.
- The front's pipelines compile on the compute device instead of sharing the offline device's: one
  more engine's compile per process, taken by the prewarm (D7 of stream-input-order).
- On a hybrid laptop the compute device and the offline device may be different cards; the front now
  runs where the codec runs, which is the point.
