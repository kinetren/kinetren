## Context

kkt-26.11.18 T1: "закрытие программы при catching up не срабатывает приходится убивать процесс". Release
kt26.11.17-2abc3af9, his iGPU (Raphael, RADV), a Datamosh loop catching up.

What his trace (`fails/26.11.18/new`, read with `kt-cli trace … summary|during|dump`) says:

- Nothing about the close. None of his three runs of that night (`new`, `old`, `render_simple_region`)
  holds "the program closes" (written at the END of `on_exit`) or any other word of a close. Two causes: no
  record existed for the close's moment or its waits, and kt-app never flushed the trace writer at a clean
  exit (`trace::flush` was called only by the panic hook), so the last batch (25 ms, `BATCH`) was lost.
  A close that hung and a kill read the same.
- What the close had to wait for: the prefetch thread's passes ran up to 37 s (`job` prefetch 36958 ms,
  `stream_advance` up to 18015 ms, a `compute_job` of 2203 ms, `gpu_admit` waits up to 1775 ms on
  prefetch and 4897 ms on a lane), the render-ahead producer's lane ran `stream_run` of 2.5 s.
  `Player::drop` joined the prefetch thread, the render thread and the build thread with no bound, then
  each media worker (the decoder finishes its whole span first: up to 64 pictures).
- After the pause at +168.6 and Diagnostics at +169.9 the UI went on drawing 2 frames a second (the
  Diagnostics clock) until the trace ends at +193.4 — 22 s with no record of a close, then the kill.

## Goals / Non-Goals

Goals: the window closes promptly in every busy state; every long job is cancellable; the shutdown is
bounded; durability (autosave, Ctrl+S, recovery, flock) intact; the trace names the close and what it
waited for. Non-goals: making the producer or the history walk faster (`ahead-kept`), the render log
(`export-readback`).

## Decisions

### D1. Every wait of a close goes through `kt_time::shutdown`

`join(who, handle, deadline)` joins a thread that ends by the deadline and leaves (detaches) one that
does not; `wait_until(who, deadline, done)` does the same for a condition; both write a `Shutdown` span
(who by `trace::name`, `dur`, `b` = left). `WAIT` = 2 s per owner's drop. A left thread ends by itself at
its next stop check or with the process. Rejected: unbounded joins with better stop checks only — a
device fence, a driver call or a sound-server call has no stop check, and one such wait is the whole
complaint again.

### D2. The exit is bounded after its durability, then `_exit`

`on_exit` keeps its order: pause, the autosave in flight (5 s), the Ctrl+S being written (30 s), the
recovery file. Then `Shutdown DURABLE`, and `exit_watch::arm(EXIT_BOUND = 8 s)`: the rest (poster 3 s,
offline engine's park, telemetry 2 s, the app's drops, eframe's teardown, the C exit hooks) must end
within it; past it the watcher records `FORCED` naming what closes left behind, flushes the trace and
calls `_exit(0)`. `_exit`, not `exit`: `exit` runs kt-gpu's hook that waits for background device work
and libLLVM's destructors under threads still drawing (row 172). The supervisor reads code 0 as a normal
close. Rejected: arming at the close's start — it could cut a Ctrl+S in its fsync; `abort` — the
supervisor would show the crash window.

### D3. Stop checks where a long job had none

- the media worker stops its span between two chunks (4 moments) once closed (it ran the whole span);
- the render-ahead producer composes each moment under `stop_when(stop::CLOSED, ctx.cancel_check())`:
  the player's drop closes its lane, and its history walk stops at the next position (`Ctx::cancel_check`
  is new; the producer's logic is otherwise untouched — `ahead-kept` works there);
- the prefetch thread's walk already stops on `left()`, which includes the player's stop; the compose
  thread's on `composer.stop()`; both unchanged.

### D4. The close is named in the journal and the trace

`note_close_asked` (once per app): the journal's `close asked` / `close asked (playing)` and
`Shutdown ASKED`; `on_exit` records each wait; `Shutdown RETURNED` after eframe returns; the trace is
flushed at the end of `on_exit` and after eframe returns. `kt-cli trace … shutdown` prints the close, its
waits, a forced exit and what each thread did after the close was asked; a run without a close says so.
`Ev::Shutdown = 120` (appended; the gap after 111 leaves room for siblings' events).

## The class walk (owner rule: a problem is a class)

The mechanism: **a wait on the close path with no bound, behind work with no stop check**. Every worker
and every wait of a close, of File → Open/New (the replaced app is dropped on its `app-drop` thread —
the same drops) and of the process's exit:

| Worker / wait | Found | Now |
|---|---|---|
| render-ahead producer (lane `near`) | lane closed at drop, not joined; a moment's history walk had no stop | walk under `cancel_check` (CLOSED); not joined (lane) |
| prefetch thread (stream / Datamosh history walks, far walk) | joined unbounded; walk stops on `left()` incl. stop, but a compute job or an admission (up to 4.9 s) has no check | bounded join (2 s), recorded |
| compose thread | not joined; its walk stops on `composer.stop()` | unchanged |
| audio render thread | joined unbounded; ends at its loop's stop check | bounded join |
| engine-build thread | joined unbounded; a build is tens of ms, ends at stop | bounded join |
| MediaWorker decoders | join waited for the whole span (up to 64 pictures) | stop between chunks + bounded join; t2952 |
| compute host jobs (Datamosh batches) | run on the prefetch/compose/producer threads; a job is not interruptible inside the device | covered by those threads' bounds and the exit watcher |
| GPU fences / arbiter admissions | a waiter on a busy ring has no stop | covered by the bounds; not changed (gpu-arbiter's rules) |
| kt-gpu exit hook (`atexit`, warm worker, compile, compute warm-up) | waited for `Busy` guards with no bound | 10 s bound, recorded; the app's forced exit skips it (`_exit`) |
| offline engine park (`on_exit`) | `wait_all` on the side/front/offline devices, unbounded | recorded; inside the 8 s exit bound |
| export (`BgJob::spawn`, its own thread) | not joined by the close; a running render dies with the process | unchanged: a render in progress is the person's choice; the exit bound covers its device work |
| thumbnails (`thumbs` lane) | lane not joined; its pump's wake under its locks deadlocked the UI at the start (row 750) | wake after the locks |
| effect cells (`FxThumbService::drop`) | `wait_until_idle` with no bound (a heavy chain's cell takes seconds) | `wait_idle(2 s)`, recorded |
| reel previews (`reel-preview` lane), posters (`posters` lane) | strip not waited; posters waited 3 s | poster wait recorded; inside the exit bound |
| LUFS / analysis (`BgJob::on_lane`) | cancel on drop, not joined | unchanged |
| index service, media watch | stop flag on drop, not joined; the media watch woke the UI under its set's lock (row 750) | wake after the lock |
| update check, telemetry sender | telemetry's exit within `EXIT_BUDGET` (2 s); update check not waited | unchanged; inside the exit bound |
| trace writer | never flushed at a clean exit | flushed after `on_exit`, after eframe, before a forced exit |
| telemetry journal writer | not joined; a line written before the exit lands | unchanged |
| audio device thread (`audio-output`) | joined unbounded; an open/close may stand in the sound server | bounded join, recorded |
| autosave in flight / Ctrl+S being written | bounded (5 s / 30 s) | recorded; durability stays first |
| File → Open / New / a recent project / a lesson while busy | `replace_with` drops the old app on `app-drop`: its player joined unbounded, doing GPU work beside the new project | the same bounded drops; the old threads stop at their next check |

## Risks / Trade-offs

- A thread left behind by a non-exit drop (Open/New) keeps running until its next check: at most one
  compute job or one decoder chunk.
- `_exit` skips the C hooks: kt-gpu's pass-time seeds written at most 30 s earlier are kept; what the
  last 30 s measured is lost only on a FORCED exit. The pipeline disk cache saves after each warm run.
- The live check runs llvmpipe in Xvfb; the owner's iGPU path (RADV fences) is covered by the bound, not
  reproduced here. His next trace with `trace shutdown` names what the close waited for.
