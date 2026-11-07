# Proposal: idle-cpu

## Why

The owner, 2026-11-07, out of turn (`docs/ktprompts/kkt-26.11.07.txt`, items E1–E5, E8): on a pause
the program holds the whole processor; a mouse move over a static place raises the load, and how
much depends on the timeline's scroll; a reopened project takes everything on hovering a
composition with Slowed, VHS and ISP, or on opening its effects. He has no tool to prove it and may
not reach a menu in such lag. An anomalous effect parameter lags the interface so that the effect
cannot be undone. "ПОЧЕМУ АГЕНТАМ НЕ ПРИКАЗЫВАЕТСЯ ТЕСТИРОВАТЬ ЭТО?!?"

Invariant 12 says the UI does not heat the system. The tests never ran a project that carries heavy
effects, and they counted percent, which on a machine with no card measures the software rasterizer.

## What Changes

**Stage 1: nothing runs at rest.**

- Work counters in `kt_time::work`: frames, composites, device submits, decode orders, prefetch
  passes, audio blocks, thumbnails, composition previews, event passes.
- The composition previews stop going round in a circle, and each release gets its own sidecar row.
- A frame that came out incomplete waits for what it lacks instead of being composed on every UI
  frame.
- The 4 Hz tick is gone: at rest no frame is asked for; a background thread wakes the UI, a clock
  asks for exactly its own frame.
- The meters falling on a pause redraw at their own 25 Hz, not at 60.
- A pointer passing over static things does not lay the events out.
- The UI and the work that is not an export run on the card that drives the monitor.
- The load watch: a thread of its own, `load-report.txt`, a section in Diagnostics.
- The guard: live scenario `idle` on `kt-cli showcase --heavy`, numeric limits on the counters.

**Stage 2: the interface never waits for a picture.** See `design.md`.

## Impact

- `kt-time`, `kt-cache`, `kt-engine`, `kt-gpu`, `kt-app`, `kt-cli`.
- Sidecar: release rows appear next to reel rows; no schema change.
- Catalogs: the `[perf]` section and `[perf.work]`, five languages.
