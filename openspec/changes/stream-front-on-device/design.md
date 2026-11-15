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

### D6. Row 561 stays open: moving the keys on a shape's readiness recomposes every kept loop

`Compiled::shape` answers `None` (the interpreter) while a shape compiles, then the specialized pass;
t1766 holds the two equal on the adapter it runs on, and the memo keys do not name the form (the
mechanism of row 510). Tried here, two ways:

- `compile_shape` calling the process-wide `forms_moved()` when a specialized pass becomes ready: the
  side device's shapes moved the display's keys too, and t2334 (cached-range-plays: a loop the fill
  kept is replayed by the display) went red — 18 of 24 moments composed again;
- a revision per device (`Compiled::shape_forms` mixed into that device's `plan_key`): still red, since
  the display's own first presents enqueue its shapes, and their readiness drops every composite the
  fill kept for it.

Both turn an unproven difference (t1766 equal here; no card) into a sure cost on every machine: each
new shape recomposes what the person's loop already kept. Reverted. The verdict of the row is now a
check on the device itself: when a shape becomes ready, run it once beside the interpreter on the
shape's first real frame (as a point-wise run's trial runs its shadow, gpu-leftovers D2) and move the
device's forms only when the two differ. Rejected: weakening t2334.

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
- `forms_revision` consumers (`plan_key`, the readback memo's keep check, the present memo): unchanged
  (D6).
- `to_trace_ns` callers: the `gpu_start` records only.

## Cost

Measured on release (`kt-cli stream-cost /dev/shm/…/behind.kt 11.2 13.2 --laps 1`) over the showcase
`--heavy --mosh-behind` (an effect → Datamosh on every styled cut), footage in `/dev/shm`, llvmpipe
(the only adapter here: compute and offline devices are both software Vulkan). The A/B is
`KT_FRONT_HELD=on|off` on the same build — `off` reads the front back from the same engine and uploads
it, the bytes form. Runs interleaved; the machine is shared with other sessions (load 4 … 78), so the
walls below come from the runs at load ≤ 30, two pairs at load 4–6 agreeing with them.

| per lap of 50 frames (62 front runs) | held | bytes |
|---|---|---|
| one front run, the advancing thread's wall, median | 0.53–0.60 ms | 2.42–2.57 ms |
| one front run, its processor time, median | 0.53–0.60 ms | 0.73–0.79 ms |
| all front runs but the first, wall | 74–80 ms | 147–154 ms |
| the walk's `source` calls, processor time (212 positions) | 62–88 ms | 95–145 ms |
| codec jobs' upload | 4.16 MB a frame | 5.31 MB a frame |
| the first front run (opening the front's engine) | 0.68–0.75 s | 0.71–1.5 s |
| processor time of the advancing thread outside jobs, a frame | 17.3–18.8 ms | 18.3–20.8 ms |

What this says:

- The round trip itself is gone: the bytes form waits ≈ 2 ms a position for the front's frame to come
  back (submit, map, copy out) and copies it twice more; the held form submits and returns. Over a
  lap that is ≈ 1.5 ms of the advancing thread's wall a frame and 1.15 MB a frame less uploaded.
- The "≈ 16 ms a position" stream-input-order measured was NOT the round trip: it is the FIRST front
  run, which opens the front's engine (0.7 s on llvmpipe with a warm disk cache, 73 s cold on a
  loaded machine), spread over the lap's 50 frames — the same in both forms. Without it the front
  costs ≈ 0.6 ms a position held. In the editor the player's prewarm (`prewarm_stream_front`, now on
  the compute device) opens it before the first walk; `stream-cost` does not prewarm.
- The stream here is coded at the kernel's own lines (the front's frames are 0.9 MB each), not at
  1080: the earlier design's "at 1080 lines" named the preview's size.
- Jobs' wall and the advance wall are the codec's on llvmpipe (44–48 ms a frame both forms at low
  load); the front was never their bound here.
- `kt-cli trace … summary` of a held run: `gpu_exec` 188 = 126 codec pieces + 62 front frames, all on
  the compute device's ring; no offline device is opened by the front.

Idle: nothing runs at rest that did not before — the front runs only inside a walk, no thread, timer
or poll was added; the spares are freed buffers (no work). The live scenario `idle` takes
`KT_LIVE_SHOWCASE=--mosh-behind` now. On the release build (`KT_LIVE_APP=target/release/kinetren`),
that project, llvmpipe: a pause runs nothing, 1 % of a core (the live control and the load watch),
trace +0 bytes, no thread of ours wakes on a clock; after a play the sound wakes nothing; an open
effects window at rest runs nothing, 1 %, trace +0. The pointer over the preview, the status bar and
the composition costs 416–445 % of a core in UI frames drawn by llvmpipe and asks no composite,
decode or job — the same verdicts as the plain `--heavy` project; the front adds nothing at rest.

## Risks

- A machine whose compute device opens but on which no engine builds (a backend that compiles the
  compute shaders but not the composite's) keeps the bytes form: `front_on_compute` returns `None`.
- The front's pipelines compile on the compute device instead of sharing the offline device's: one
  more engine's compile per process, taken by the prewarm (D7 of stream-input-order).
- On a hybrid laptop the compute device and the offline device may be different cards; the front now
  runs where the codec runs, which is the point.
