# Design: export-readback

## Context

The owner's trace `fails/26.11.18/render_simple_region` (kt26.11.17-2abc3af9): one 1080p60 event, no
effects, exported on his Ryzen 7700X iGPU (RADV RAPHAEL_MENDOCINO, the display's adapter) with
`h264_vaapi`. `kt-cli trace … export`: 2238 frames, 37.29 s of timeline in 39.61 s, 0.94× real time, one
device (a card), the readback ring of `export-throughput` D5.

## The mechanism (from his records, no bench)

| record | what it says |
|---|---|
| `GpuFrame` aux 2 (export), 2237 | the device's passes took 1.06 ms a frame (p50, p90 2.66 ms, all 3.46 s) |
| `GpuAdmit` / `GpuExec` per frame | admitted at once (30.7 ms in all); 1.5 ms of predicted work in flight before each |
| `job` `export_frame` (the ring's `collect`) | 15.0 ms a frame (p50), the gpu wait 82 % of the wall |
| `GpuExec` → end of its `collect` | 52.7 ms from a frame's submission until it was collected (p10 43.7, p90 60.3) |
| collect end → next submit | 0.87 ms |

The compose thread submits frame N+2 and then waits for frame N. Frame N was submitted 36 ms earlier (two
frame periods) and still is not back: it takes ~50 ms from submission until the device signals it,
whatever the work. The ring holds `RING` = 3 frames, so the export's rate is 3 / latency = 57 frames a
second — the 0.94× of a 60 fps project. Nothing in the export is busy: the device 6 %, the writer waiting
for frames 93.5 %, the frame workers idle 373 % (four of them), the decoder held by the look-ahead 86 % (the
written frames free it).

Where the 50 ms come from on his machine is not in the records (the trace has no device-side start of a
graphics frame; the device shares the display, the compositor and the editor's own UI, and amdgpu may park
an iGPU's graphics block between short bursts). It does not matter for the fix: a frame's latency is not its
work, and a fixed ring turns latency into rate. `export-throughput` D5 chose three ("one being read back,
one being rendered, one being built") on llvmpipe, where latency equals work.

Our llvmpipe export of a simple 1080p60 region (four devices, a frame in flight per worker): 2.22× real time
at load 18, 0.34× at load 46 (siblings' builds); copy out 1–3 ms a frame. A software adapter has no latency
beyond its work, so this change does not move it; the card path is the owner's.

## Decisions

### D1. The ring's depth is bounded by bytes; the frames in flight follow the device

`readback_depth(w, h)` answers how many frames of that size the ring may hold: its staging budget
(`RING_BYTES`, 256 MiB) over the padded frame size, at least `RING_MIN` (3, the former constant), at most
`RING_MAX` (16). 1080p: 16 (133 MB), 4K: 7 (232 MB), 8K: 3. The slots are made as they are first used (up to
the depth, at the export's start) and reported to the memory ledger as readback memory; nothing is made per
frame after (t2023's rule).

The depth is a bound, not the depth used. The compose thread, after each submission, hands on every frame
at the ring's front that the device has finished (`VideoGpuExec::done(ticket)`: a non-blocking poll and the
map's verdict) and blocks in `collect` only while the ring is full. A device that hands a 1 ms frame back
after 50 ms keeps about latency / submission interval frames in flight; one whose frames come back at once
keeps one or two, and its readbacks start sooner than before (the old loop collected only at three). The
number is measured by the device every frame; no constant is tuned for a machine.

Bytes: the same plans, the same passes, the same order on the one queue; the ring's slots are reused only
after their frame is collected. The arena, the pool's accumulators and the reused upload textures are written
by `queue.write_*`, ordered after every submission already reading them (export-throughput D5) — that holds
for any depth. The uploads an export device keeps (`UploadCache::keep_frames`) stay three frames: a frame
deeper in the ring keeps its textures alive through wgpu's references, and no later frame reads them again.

The pass timer kept at most 8 frames waiting for their timestamps; past it a frame ran untimed. With 16 in
flight that would blind the `GpuFrame` records exactly when they matter: its buffers are made for
`PENDING_MAX` = 32 frames, all at the timer's creation (8 KB each).

Rejected: a depth computed from measured latency (`ceil(latency / interval)`): the readiness rule gets the
same depth without a clock steering the export, and needs no warm-up; a collector thread apart from the
submitter: one thread already hands on done frames between submissions, and a second one would contend for
the engine's lock; collecting out of order: the writer and `collect` stay in submission order (frame order).

### D2. The trace states the ring

Two `ExportStage` records join the stages: `RING` (`a` the depth bound, `b` frames in flight summed over the
submissions, `c` the submissions, `id` the most in flight at once) and `LATENCY` (`a` submission-to-collected
nanoseconds summed, `b` the longest, `c` the frames). They are not stages of the busy list. `kt-cli trace …
export` prints, per export:

- `device: 1.06 ms a frame of work (p50 of 2237 timed frames)` from the export's `GpuFrame` records;
- `readback: up to 16 in flight of 16 (mean 11.2), a frame back 52.7 ms after its submission` from the new
  records, or for a trace written before them `readback: a collect waited 15.0 ms (p50), ring of 3` from the
  `export_frame` spans;
- the run's `EXPORT` journal lines between the export's start and end (D3).

The profile's line in the journal (`summary`) names the ring too.

### D3. The render log is the telemetry's

`push_log` already wrote every line into the telemetry journal (`Kind::Export`), which mirrors it into the
trace (`trace::journal_at`, structured-telemetry). `render.log` repeated the same lines in a third place, and
`render.log.1` the run before. Both go: `LogFile` and `RenderSetup::log_dir` are removed, the window keeps
its 800 lines (open by itself after an error), the path line and "the log is only in the window" go with
their catalog keys, and the failed render's **Open log** opens the telemetry journal. A file the program
wrote before is left where it is (the data directory is the person's); nothing reads it.

### D4. Not done: YUV on the device, a VAAPI surface

The task named handing a hardware encoder a device surface, or converting to YUV on the device, so the host
copy (12 % of the wall) and the CPU `convert` (26 %, over four workers) shrink. In his trace neither bounds
the export: the four convert workers were idle 373 % of the wall, and the copy is 2.2 ms a frame on the
compose thread. With the latency hidden, the next bound on his machine is the compose thread's own 3.1 ms a
frame (plan, submit, copy out) — about 320 frames a second, 5× his project's rate — then the encoder. A
device surface for VAAPI needs a dma-buf exported from wgpu's Vulkan device into libva, which wgpu does not
expose; a YUV pass on the device moves the conversion but keeps the copy. Recorded as found-defects 741 for a
measurement on his machine after this change, not built blind.

## The class: every path that reads a frame back

The defect's mechanism: a readback that waits a fixed number of frames behind (or synchronously) where the
device's latency, not its work, decides the rate. Checked:

| path | readback | verdict |
|---|---|---|
| export, one device (a card) — `submit`/`collect` | ring of 3, waited in order | **the defect**; D1 |
| export, several software devices — `execute_once` per worker | synchronous per worker, one frame per device in flight | latency = work on llvmpipe; N workers overlap N frames; the device rule counts them by measured rate (render-honest D1). Unchanged |
| serial export (`ExportOptions.serial`) — `execute_once` | synchronous | the reference; speed not its contract. Unchanged |
| the export's incomplete-frame retries (`execute_once` inside `collect`) | synchronous, rare | a frame missing a layer; unchanged |
| preview present (`present`, `present_kept`) | none: the picture stays on the device | not a readback |
| render-ahead producer, same device (`ahead` → `keep_tex`) | none: kept on the device, a done flag | not a readback (and `ahead-kept`'s area) |
| render-ahead bytes route (`ahead_by_bytes`: `execute_into` + `keep`) | synchronous, one moment | only a display with no sibling or `KT_RENDER_AHEAD_DEVICE=side`; its depth is the producer's own (render-ahead D4). Unchanged, noted |
| the GL preview off the display, `KT_PREVIEW_READBACK` (`execute_into`, `read_kept`) | synchronous, the frame the person waits for | one frame asked, one waited; no throughput to gain. Unchanged |
| effect cells, reel strips, posters (`side_engine`, `execute_into`) | synchronous, one frame a job | a lane job per frame, class Visible/Background; unchanged |
| stream front on the compute device (`execute_held`) | none: held on the device | not a readback |
| Datamosh history (compute jobs, `run_keeping`, the reference walk's bytes) | a job waits for its own submission (`wait_for`, D9 of datamosh-history-wait) | one job a batch; its reads are mapped first. Unchanged |
| the pass timer's timestamps (`harvest`) | never waits | its pending frames raised to cover the ring (D1) |
| compute host calibration and warm-up | wait for their own submission | once per 10 s / once. Unchanged |

Only the export's ring had a depth that turned latency into rate.
