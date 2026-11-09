# Design: gpu-arbiter

## Context

Scheduler stage 1 (`one-scheduler`) gave the program job classes (`kt_time::sched::Class`), lanes and
pause votes for CPU work. The GPU side has no arbiter (audit §2b):

- the display device (eframe's, shared with egui) runs the preview's present (`compose_queue`, one
  composite at a time, then `wait_presented`, a 1 ms poll);
- one offline device (`kt_gpu::offline_engine`) runs the slow-chain preview route (a chain past
  `SLOW_CHAIN` = 150 ms), effect cells, reel strips and posters, one `inner` mutex per engine;
- the export opens its own devices (one on a card, 2–4 on a software adapter) and keeps a ring of
  three frames in flight (`submit`/`collect`);
- one compute host per process (`compute.rs`) runs Datamosh's jobs for the compose thread, the
  prefetch thread (`stream_advance_ahead`), reel/poster pre-roll and the export, each `run` submitting
  and blocking with `poll(wait_indefinitely)`; nothing orders the callers.

On a hardware card these are separate `wgpu::Device`s on ONE graphics ring: the driver interleaves
them by submission, so a 300 ms offline submission stands in front of the display's frame and of
egui's own (row 149). llvmpipe/lavapipe hides it: every device has its own queue thread, so the bench
reproduces the ring by putting the display, the offline engine and the compute host on ONE device.

wgpu facts that shape the design (wgpu-core 30, read in `device/resource.rs`, `present.rs`):
`device.poll(Wait)` holds the device's snatch lock (read) for the whole wait; `Surface::present` takes
it for write. So a blocking wait on the display device holds egui's present — that is why
`wait_presented` polls. A blocking wait on any other device holds nobody: no surface lives there.
`on_submitted_work_done` callbacks fire during any maintain of that device, which every
`queue.submit` runs.

## Goals / Non-Goals

**Goals:**
- One ring per physical queue, class-ordered admission, a bound on what a ring holds in front of the
  display, long lower-class frames and compute jobs cut into measured pieces.
- Every submission tagged with the class of the work, counted and traced; `kt-cli trace gpu`.
- A stream history stops when nobody wants it (plugin API, additive).
- The preview's present waited for without a fixed-period poll; a paused export without a timer.
- Per-render decode settings.
- A bench with numbers before/after on llvmpipe from the structured trace.

**Non-Goals:**
- Preempting a submission already on the device (impossible in wgpu); a single pass longer than the
  bound stays one piece.
- Fewer Datamosh jobs per history position (merging encode and decode in one job, packed readbacks):
  that is `stream-layout-per-build`, running beside this change in `fx-video-codec/codec.rs`; this
  change touches only `mosh.rs`'s loops for the stop check.
- The memory ledger (scheduler stage 3), the memo against loop length (row 143).

## Decisions

### D0. The mechanism and the walk (the class)

Mechanism: **a resource two subsystems share has no owner that orders their use, so the one that
came first holds the other for as long as its own unit of work lasts, and a unit nobody wants any
more still runs to its end.** Walked across every subsystem that shares a resource of this kind:

| Where | Shared resource | Found | Verdict here |
|---|---|---|---|
| preview present (`engine.rs` `present`) | display ring | the display is the protected party; short by construction (slow chains leave it) | admitted always, never cut; waits without polling (D8) |
| slow-chain route (`compose_queue` → offline `execute`) | card ring with the display | a 150 ms+ chain in one submission (row 149) | class now; cut on a displayed ring (D4) |
| effect cells, reel strips, posters (offline engine) | card ring; one `inner` mutex | whole frames in one submission; the slow route waits on the mutex for a cell | classes visible/background from their lanes; cut on a displayed ring. The mutex stays: a frame is atomic for the engine's arena; the cut shortens each hold on the ring, not on the mutex (row 290) |
| export (`submit`/`collect`, `execute`) | card ring when the editor renders on the same card | three frames in flight | class offline; admitted and bounded on a displayed ring, unbounded alone (kt-cli, software devices) |
| compute host `run` | its own queue (and the card ring) | Datamosh jobs of compose, prefetch, pre-roll, export in arrival order (row 134); generator frames on media workers | admitted by class; a job cut between dispatches on a displayed ring (D4) |
| media workers' generator compute frames (`kt-plugin-api` generator) | compute host | class unknown | class from the request's priority: playback → now, otherwise near |
| warm worker (`warm.rs`), timer readbacks | the device | one 6×6 draw per submission after `wait_frames`; a copy | short by construction; not admitted (recorded) |
| stream history (`StreamKernel::render`) | compute host, a core | a frame the UI left runs its whole history (row 147) | stop check (D7) |
| prefetch's stream advance | compute host | runs ahead after a seek | stop when the playhead leaves (D7); class near |
| decoder settings (`kt_ffmpeg` statics) | process globals | the render window sets them around its open; a preview open meanwhile takes them (row 94) | scoped to the opening thread (D10) |
| other process-wide setters, walked: `kt_gpu::pipeline_cache::set_cache_dir`, `kt_plugin_api::compute::install`, `set_decode_threads` in kt-cli | process globals | set once at start by the product | no defect (one writer, before any reader) |
| `wait_presented` | display device's lock | 1 ms poll (row 272) | D8 |
| export pause (`export.rs`, `export_pipeline.rs` Gate) | a thread | 40 ms sleep, 50 ms timed wait (row 271) | D9 |
| `finish_present`, `park`, `read_display` | display device | blocking waits | test/exit paths only (`preview_bench`, exit, tests); recorded, unchanged |

### D1. A ring per physical queue

`arbiter::ring_for(info, device)`: a software adapter (`DeviceType::Cpu`) gets a ring of its own per
device (its own queue thread); a hardware adapter with a PCI id gets the ring of `(vendor, device)`;
a hardware adapter reporting no id (some GL drivers) gets the ring of the first hardware ring opened,
or a ring of its own if none — merging two cards costs throughput, never a picture. The editor's
display device marks its ring `display` (the bench's display engine too). Rejected: one ring per
`wgpu::Device` — the very defect; one ring per process — llvmpipe's independent queues would wait
for each other for nothing.

### D2. Admission

A ring keeps the pieces in flight (class, predicted ns, device, submission index, a done flag the
work-done callback sets) and the waiters by class. `admit(class, predicted)` returns at once when no
higher class waits and either the ring has no display, or nothing is in flight, or in-flight + predicted
≤ bound. Otherwise it waits: it completes the oldest in-flight piece itself with a blocking
`poll(Wait{index})` on that piece's device when that device carries no surface (any non-display
device), else on the ring's condvar. So no piece depends on its own submitter returning to collect it
(the export's ring of three would deadlock otherwise). The display's present is never admitted: it
is the party the bound protects. On a ring without a display admission still orders the waiters by
class (the compute host on llvmpipe: the frame on screen before the look-ahead), but holds nothing.

### D3. The bound

Half a frame period of the preview's project (`arbiter::set_frame_period`, from the player's fps; 40 ms
→ 20 ms by default): the display's frame and one piece then fit one frame period. A general rule, not
a measured number of one machine; what is measured on this machine is where the cuts fall (D4).

### D4. Pieces cut by measured pass time

The engine keeps, per pass label, the device time the pass timer last measured (`Timer::harvest`
feeds `Inner::pass_ns`); the compute host keeps it per dispatch name. Before encoding a pass of a
bounded frame (lower class, displayed ring) the engine asks `piece.before(predicted)`: when the piece
is non-empty and the sum would pass the bound, the current encoder is finished and submitted as a
piece, the piece's completion is awaited (blocking poll on its own non-display device), and the next
piece is admitted. A pass never measured ends its piece (conservative). The frame's timestamp
queries are resolved in its last piece. Bytes: the same passes in the same order into the same
textures; `queue.write_buffer` slots are distinct per pass, so a write after a cut never lands under
an earlier piece. The compute host cuts a job between dispatches the same way; its buffers live
across pieces. Rejected: cutting by pass count (a resample and a Camera ISP pass differ 100×);
cutting at job boundaries only (VHS is one job of 14 passes).

### D5. The class of a submission

`kt_time::sched::current_class()` — thread-local, set by `with_class(c, f)` / a guard; the scheduler's
worker sets it to the lane's class around a job. The compose thread runs as `now`, the prefetch thread
as `near`, the export's threads as `offline`, a media worker per request (playback → now, otherwise
near). Untagged → background: an unknown long job must not pass the frame on screen.

### D6. Trace and query

New `Ev` rows (appended): `GpuAdmit` (aux class, a predicted ns, b waited ns, id ring), `GpuPiece`
(a piece index, b predicted ns, cause the frame/job), `StreamStop` (id clip, a position, aux who:
compose/prefetch/job). `kt-cli trace gpu`: per ring and class, admissions/s, total and max waits,
pieces, compute jobs and device submits/s, the present latency (`FrameWait` device) p50/p95/max.

### D7. Stop check (plugin API, additive)

`StreamInput::stop(&self) -> bool { false }`. Datamosh asks it before every history position in
`broken` and `clean_at` (one or two compute jobs) and returns `None`; progress is in the memo, so
the bytes of a later render are those of an uninterrupted one (invariant 11: the memo is speed only).
The host's `ClipStream::stop` reads a thread-local stop check (`video_stream::stop_when(check, body)`):
the compose thread's check is "a newer ask waits for another moment, an edit changed the versions, or
the composer stops"; the prefetch thread's "the transport's generation moved or the project version
changed since the pass began"; a pre-roll job's "its token is cancelled". An export sets none. A
stopped run is traced (`StreamStop`) and reported like a missing frame (incomplete), never kept.

### D8. `wait_presented` without polling

The frame's expected device time is the timer's last measured preview frame (no timestamps: the last
submit-to-done; nothing measured: 2 ms). The wait registers `on_submitted_work_done` that notifies a
condvar, waits on it for the expected time (any submit on the device, e.g. egui's frame, runs the
maintain that fires it early), then makes one non-blocking poll; while not done it waits half the
remaining guess (≥ 1 ms) and checks again. The count of checks is a counter (`Work::PresentCheck`).
Rejected: a blocking wait (holds egui's present, see Context); a 1 ms poll (row 272).

### D9. Export pause on events

`wait_paused` waits on `kt_time::sched::wait_gate(|| paused && !cancelled)`; whoever sets the pause or
the cancel flag calls `notify_gate` (`kt_engine::export::set_paused`/`cancel` helpers, the render
window uses them). The pipeline's `Gate::wait` drops its 50 ms timeout: the halt flags are set
through `Gate::wake`.

### D10. Per-render decode settings

`kt_ffmpeg::with_decode_settings(DecodeSettings{decode_threads, rgba_threads}, f)` sets a thread-local
read by `decode_threads()`/`rgba_threads()` at a source's open (the open is synchronous on the caller's
thread, and the source keeps the values for its reopens). The render window wraps its `open_all` in
it; kt-cli keeps the process-wide setters (one process, one render).

### D11. The bench

`kt-cli gpu-share <project.kt> [--seconds N] [--arbiter on|off|both] [--display S] [--offline S]
[--mosh S] [--size WxH] [--check]`: one headless device; a display engine (`register_display`)
presents 50 frames of the project from `--display` (a range with no stream stage: the bench refuses
one, the display would compute a history instead of presenting), one every frame period, and records
each present's device wait as a `FrameWait`; an offline engine on the same device composes the
moment at `--offline` again and again (class offline); a compute host on the same device advances the
Datamosh history from `--mosh` on (class near). `kt-cli showcase --mosh` lays a Datamosh on the styled
montage cuts of the showcase. Test t2254 runs a synthetic version (VHS Tape + Camera ISP at 1080p and a
spinning compute job) and compares the two modes inside one run; its clock reads are a comparison of
two runs, not a bound of a wait (t1671 does not list it).

### D-measure. Before and after (release kt-cli, llvmpipe, machine load 30–110 from other sessions)

- **Baseline, before any code** (`kt-cli preview` of `showcase --mosh` 10–17 s, `--trace`): 93 compute
  jobs in 7 s, every one on the prefetch thread (13.3/s); the compose thread ran none (the pacing keeps
  histories off it). V-fps 0.7 at load 82 — a load figure, not the program's.
- **Row 149 on ONE device** (`kt-cli gpu-share` on `showcase --mosh`: display 30 s, offline the VHS
  Tape cut at 76 s, look-ahead from the Datamosh at 11.2 s; 1080p; 8 s each, after a 2 s warm lap):

  | | display device wait p50 / p95 / max | presents in 8 s | offline frames/s | history positions/s | pieces |
  |---|---|---|---|---|---|
  | arbiter off | 32.1 / 322.1 / 763.7 ms | 63 | 4.38 | 2.9 | 0 |
  | arbiter on | 26.6 / 50.3 / 73.3 ms | 193 | 4.50 | 26.6 | 395 |

  The offline frames keep their rate: a piece waiting for the one before costs a wake-up, and the
  display's frames slip in between. The look-ahead ran faster with the rings (its jobs no longer sat
  behind whole offline frames). `kt-cli trace … gpu` over the same run: ring 1 carries the display;
  near 273 admitted, waits max 55 ms; offline 716 admitted, 639 pieces cut.
- **Synthetic, in the test** (t2254: VHS Tape + Camera ISP at 1080p offline, a spinning compute job,
  4 s each, load ~90): display wait p50 328 → 93 ms, p95 772 → 208, max 1018 → 229 ms; 8 → 40 frames.
- **A heavy frame cut**: t2253 cut VHS Tape + Camera ISP at 1080p into 22 pieces at a 20 ms bound,
  same bytes.
- **The present's wait** (t2260): 19 checks over 10 presents of 107 ms each (was a poll a millisecond:
  ~1000).
- **Row 134**: the same preview run after: 12.3 jobs/s with the rings, 12.9 without (`KT_GPU_ARBITER=off`),
  all class near — the rings order the look-ahead behind the frame on screen and stop a history the
  playhead left, but the count is the stage's own (two jobs a position); fewer submits are
  `stream-layout-per-build`'s (packed jobs). V-fps 20.1 vs 19.8.
- **Rest**: live `idle`, `sched_pause`, `heavy_loop`, `ui_wait` (section 5 of tasks).

## Risks / Trade-offs

- [Throughput of a lower class on a displayed ring] → a piece waits for the one before, so the
  device idles between pieces for a wake-up; measured by the bench (offline frames/s with and without).
- [Prediction from mixed sizes per label] → a label's last time can come from another size; a wrong
  prediction moves a cut, never a byte; the bench shows the pieces' real device time.
- [Timestamps resolved in a later submission] → checked by the timer's own records in t2253; on a
  failure the frame is recorded untimed, the picture is unaffected.
- [A GL display with no PCI id on a two-card machine] → may share a ring with the wrong card: costs
  throughput only (row 291).
- [Stop returns `None`, which a caller reads as "missing"] → the compose thread drops such a frame
  (its ask was replaced); a pre-roll stops only when cancelled.

## Open Questions

None that change what is built. The bound's rule (half a frame period) is a general rule the owner
may want to revisit; it is a constant in one place.
