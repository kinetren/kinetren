# Proposal: one-scheduler

## Why

Row 4 of the order table in `docs/engine-audit.md` (scheduler stage 1, mechanisms M3, M4, M6, M10),
started on the owner's standing go (kkt-26.11.09 I10/I11: "когда эти закончат - продолжай другие
делать по таблице"; "не надо меня ждать"). He agreed that background services pause while playing
(kkt-26.11.08 H15, "Да") and forbade tuning to one machine (H13): limits are general or measured at
run time.

The class: background work runs on many ad-hoc threads, each with its own queue discipline and its
own timer. Nothing knows what is running, nothing pauses all of it while playing, nothing cancels it by
one rule, and some of it still waits on the UI thread. §2b of the audit lists about ten queue
disciplines and seven timers that run at rest; `found-defects.md` rows 98 (Ctrl+S writes and syncs on
the UI thread), 114 (thumbnail and effect-cell caps go round), 122 (the load watch wakes once a
second), 140 (repaints after a pause on the live bench), 148 (the UI thread opens files and joins a
decoder on media gestures) and 174 (the thumbnail worker wakes every 2 s) are instances.

## What Changes

- **`kt_time::sched`: one executor.** Job classes in a fixed priority order (now, answer, near,
  visible, background, offline), lanes (a producer's FIFO, one job at a time; one-shot jobs or a pump
  over the service's own queue), a small pool sized from this machine's cores at run time with one
  worker always kept for the person's answers, cancellation tokens, pause votes (a playing player, an
  export) that stop the pausable classes and hold their checkpoints on a condvar, per-class counters,
  and trace records for every post, run, cancel, pause and resume. Workers park on a condvar: nothing
  wakes on a clock.
- **`kt_time::block`: a counter of blocking calls** on the UI and audio render threads (file open,
  fsync, join, build, sidecar, write), traced with their length; a test requires zero over the
  person's media and save gestures.
- **The services move onto it**: thumbnails, effect cells, posters, reel previews, analysis, the
  index, the media watch, autosave and Ctrl+S, the sources reopened when a file returns, the max
  source size, the poster loader. Each loses its thread and its timer.
- **Timers at rest go**: the audio render thread's 250 ms and prefetch's 100 ms on a pause, the
  index's 200 ms, the watchdog's 900 ms (now event-driven: open, focus, play/stop, input), the load
  watch's second (it parks after a quiet stretch and re-arms on work or input), the thumbnails' 2 s,
  the effect cells' 1 s and 25 ms, `BgCtl`'s 30 ms pause poll, telemetry's heartbeat on a quiet
  session.
- **The UI thread's waits become jobs**: Ctrl+S, media opens on gestures (`add_media_clip_at`,
  `place_media_at`, `sync_media_sources`), the decoder join of `replace_media`/`remove_media`.
- `kt-cli trace during` names every job by class and lane, pauses and resumes, blocking calls.

## Capabilities

- `media-index`: the services and the offline path's rule run through the scheduler; background
  classes pause while playing.
- `transport`: a pause parks the render and prefetch threads with no timer.
- `telemetry`: scheduler and blocking records; the load watch parks at rest.
- `durability`: Ctrl+S writes off the UI thread; long background work is a job with a token.

## Impact

`kt-time` (new modules), `kt-engine` (player threads, pause vote, joins off the caller), `kt-app`
(every service), `kt-cli` (trace query). No file format change. Invariants 5, 6, 12 hold (design D3).
