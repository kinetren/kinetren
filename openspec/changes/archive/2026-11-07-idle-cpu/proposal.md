# Proposal: idle-cpu

## Why

The owner, 2026-11-07, out of turn (`docs/ktprompts/kkt-26.11.07.txt`, item E1): on a pause, with
nothing happening, the program holds the whole processor. Invariant 12 says the UI does not heat the
system. The tests never ran a project that carries heavy effects.

The other requests of that list are changes of their own (`kkt-26.11.07-index.md`).

## What Changes

- Work counters in `kt_time::work`, read by the geometry dump.
- The composition previews stop going round in a circle, and each release gets its own sidecar row.
- A frame that came out incomplete waits for what it lacks.
- The 4 Hz tick is gone: at rest no frame is asked for. A background thread wakes the UI, a clock asks
  for exactly its own frame, and a gesture gets one more frame to show what it did.
- The meters falling on a pause redraw at their own 25 Hz, not at 60.
- `kt-cli showcase --heavy` and `kt-cli pause-scan`, to run all this on a project of our own.

## Impact

- `kt-time`, `kt-cache`, `kt-engine`, `kt-gpu`, `kt-app`, `kt-cli`.
- Sidecar: release rows appear next to reel rows; no schema change.
