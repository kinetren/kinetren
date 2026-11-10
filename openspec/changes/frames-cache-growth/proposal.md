# Proposal: frames-cache-growth

## Why

The owner's run of stage 9 (`kkt-26.11.10` J4, his trace `fails/26.11.10/trace`): "кэш странно
работает, внизу показывает 39/535 MB ... если программа жрет 4.7гб, то почему тут такие цифры? и
почему 535 а не больше? и даже смена качества превью сильно не заставляет его повысить потребления.
... video cache usage: 100% (535 / 537 MB)". Diagnosed from his trace as found-defects 396 and 397:

- **396.** `memory-ledger` gave the process one growth budget, but the decoded-frame cache never asks
  for any of it: `BurstCache` opens its `Frames` accounts with the configured budget as base, never
  calls `want()`, and evicts against `config.max_video_bytes`. The look-ahead window
  (`prefetch.rs`, half the budget), the decoder's span room (`worker.rs`, half) and the stream pacing
  (`player.rs`, a quarter) derive from the same constant, so the preview quality and a loop change
  nothing: his trace shows the frames cap at 576 MB the whole run while ~21 GB of growth budget stood
  free, eviction spikes of 537/s and prefetch thrash.
- **397.** The status bar's "cache: 39/535 MB" is audio MB / video MB, read by a person as used/limit;
  Diagnostics shows video used/limit only; 4.7 GB of the process are named nowhere — the ledger
  accounted 1.4–3.7 GB, the rest (decoders inside FFmpeg, the graphics driver, the interface) has no
  number, and the trace has no record of the process's memory or of any tenant's bytes.

## What Changes

- **The preview's decoded-frame cache asks the ledger** for what its look-ahead and its loop need at
  the current preview quality and decode step: the window's peak bytes twice over (the window takes
  half the cap), or the whole loop region's frames plus an eighth and the window. The cache evicts
  against the cap it was granted (base + its grant); a grant taken back (an export's claim, a higher
  tenant) evicts at once through the ledger's shrink hook, its own lower want evicts in the asking call.
  Holds and pins stay stronger than any cap. A cache that never asks (an export's, a reel strip's)
  stays at its base; a cache that is dropped gives its want back.
- **What reads the budget reads the cap**: the look-ahead's half, the decoder's span room and the
  stream pacing's quarter follow the cap as it moves (the pacing through a shared value read at use,
  the renderers being shared builds).
- **The numbers say what they count**: the status bar reads "frames: V/cap MB" and "memory: P MB"
  with a hint; Diagnostics shows the decoded frames against their cap now, the caches' line with the
  kept mix, and its first row "Program P MB · rest R" (the program's resident memory and what no cache
  counts, the rest named by its hint as the decoders, the graphics driver and the interface). Five
  catalogs.
- **The trace carries memory**: `frames_cap` (the preview cache's cap, the window's and the loop's
  bytes, the quality), `mem_sample` (the process's resident memory, the ledger's sum, the memory
  available) and `mem_used` (a tenant's bytes and cap) — written once per busy second when a number
  moved, never at rest. `kt-cli trace memory` prints them.
- The class walk (every size derived from an old constant instead of the ledger's grant) is in
  design.md.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: ADDED — the preview's decoded-frame cache grows by the ledger for its look-ahead and
  loop; the readers of its budget read the cap.
- `telemetry`: ADDED — the process's memory and every tenant's bytes in the trace, once per busy
  second when they moved; the frames cap sizing.
- `ui-timeline`: ADDED — the status bar's and Diagnostics' memory numbers say what they count and
  name the whole process.

## Impact

`kt-cache` (cap, want, shrink hook), `kt-engine` (prefetch sizing, span room, pacing), `kt-time`
(trace kinds 93–95, `mem::sample`, resident probe slot), `kt-gpu` (`memory::resident`), `kt-app`
(status bar, Diagnostics, load watch sample), `kt-cli` (preview sample, `trace memory`). Determinism:
a cap decides only what is kept (invariant 11). Tests t2390–t2409.
