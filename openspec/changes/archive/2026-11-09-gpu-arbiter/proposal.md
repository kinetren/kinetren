# Proposal: gpu-arbiter

## Why

Row 5 of the order table in `docs/engine-audit.md` (scheduler stage 2, mechanisms M5 and M4), started
on the owner's standing go (kkt-26.11.09: "когда эти закончат - продолжай другие делать по таблице";
"не надо меня ждать - запускай сам по мере завершения все сам"). He forbade tuning to one machine
(kkt-26.11.08 H13): every limit here is a general rule or is measured at run time.

The class (M5): **two subsystems share one device with no arbiter, and a long submission of one holds
the other**. The GPU work of the program comes from the preview's present, the slow-chain route and the
effect cells/reel strips/posters on the offline device, the export's devices and the compute host
(Datamosh's history from the compose thread, the prefetch thread, the reel/poster pre-roll and the
export). Nothing orders them: on a real card every one of those devices feeds the same hardware ring,
so a long offline submission stands in front of the display's frame (`found-defects.md` row 149), and
the compute host runs whoever submitted first, so the frame on screen waits behind the look-ahead's
Datamosh jobs (row 134: ~45 submits/s from prefetch far ahead of the cursor). A stream history the UI
has left cannot be stopped (row 147: the preview's thread held 4 cores for seconds after a pause). A
render opened from the window sets process-wide decoder thread settings that a preview source opened
meanwhile takes (row 94). Two waits poll: the preview's present every 1 ms (row 272), a paused export
every 40–50 ms (row 271).

## What Changes

- **One arbiter per ring** (`kt_gpu::arbiter`): every device of the program registers on the ring of
  its adapter (a hardware adapter's devices share one ring, whatever their backend; a software
  rasterizer's device is a ring of its own, because it runs on its own queue thread). A submission of
  anything but the display's present is ADMITTED by the ring in class order (`kt_time::sched::Class`:
  now, answer, near, visible, background, offline), and while the ring carries a display, the device
  time a ring holds in front of the display is bounded.
- **One submission bounded by measured device time**: on a ring that carries a display, a frame of a
  lower class is cut at pass boundaries into pieces whose device time, as the pass timer measured those
  passes on this machine, stays under the bound (half a frame period of the preview; a pass longer than
  that is a piece of its own); a piece waits for the one before it. The same bytes come out (the
  passes, their order and their targets do not change). The compute host's jobs are admitted the same
  way.
- **Every GPU and compute submission carries a class**: a thread-local class set by whoever runs the
  work (the compose thread: now; the prefetch thread: near; a scheduler lane's job: its lane's class;
  the export: offline). `kt-cli trace gpu` shows submits per second, admissions, waits and pieces by
  class and by ring.
- **A stream history stops when nobody wants it** (plugin API, additive): `StreamInput::stop()`; the
  Datamosh kernel checks it before every history position (one or two compute jobs), keeps what it
  computed in the memo and returns. The host answers from a stop check its caller set: the compose
  thread stops a frame a newer ask or an edit replaced, the prefetch thread a pass the playhead left,
  the reel/poster jobs a cancelled token. An export never stops.
- **The look-ahead's history yields to the frame on screen**: prefetch's stream advance is class near,
  so the compose thread's jobs are admitted first on the compute host; the advance is measured before
  and after in jobs per second from the trace.
- **The preview's present waits without polling**: `wait_presented` sleeps on the work-done callback
  for the device time the timer measured for that frame, then checks once; no 1 ms loop (row 272).
- **A paused export waits on the scheduler's gate** (`kt_time::sched::wait_gate`), and the pipeline's
  write gate on its condvar with no timeout (row 271).
- **Per-render decode settings** (row 94): `kt_ffmpeg::with_decode_settings` scopes the decoder and
  RGBA thread counts to the sources the render opens on its own thread; the process default stays for
  everyone else.
- A bench: `kt-cli gpu-share` runs the preview's present and an offline job (and a Datamosh history)
  on ONE device of the machine and reports the display's latency with and without the arbiter, from
  the structured trace; `kt-cli showcase --mosh` lays a Datamosh history on the showcase for it.

## Capabilities

### New Capabilities
- none.

### Modified Capabilities
- `video-plan`: one ring arbiter for every device; bounded submissions on a ring with a display;
  the preview's present waits without polling.
- `plugins`: `StreamInput::stop` — a stream kernel stops between history positions when the host
  says nobody wants the frame.
- `export`: a paused export waits on an event; a render's decoder settings are its own.
- `telemetry`: admission, piece and stop records; `kt-cli trace gpu`.

## Impact

`kt-gpu` (new `arbiter.rs`, engine.rs encode/submit paths, compute.rs, the bench's shared-device
constructors), `kt-time` (`sched::current_class`/`with_class`, trace events), `kt-plugin-api`
(`StreamInput::stop`, additive), `fx-video-codec` (the stop check in `mosh.rs` only), `kt-engine`
(stop scopes in the compose thread, prefetch and pre-roll; export pause), `kt-ffmpeg` (scoped decode
settings), `kt-app` (`render_dialog.rs`, tags), `kt-cli` (`gpu-share`, `showcase --mosh`, `trace gpu`).
No file format change; no `.kt` change; invariant 11 holds (timing only).
