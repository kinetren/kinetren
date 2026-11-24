# Proposal: durable-history-and-mix

> DESIGN (spec session). Revised 2026-11-24 for the owner's D6 (kkt-26.11.22 line 9): the Datamosh history is
> NOT stored on disk; only final frames are kept, and on a miss the history is recomputed from the cut. This
> change now carries the kept mix's disk tier and the restart line only; the name is kept so the references to
> it stay valid. The owner's A14 (kkt-26.11.20 line 19) still decides the mix half: build now, in the project's
> cache folder. Tests: t3336, t3337, t3346–t3352 (t3338–t3345 and t4105, the history tier's, are freed). Base:
> orch `af14db35`.

## Why

render-cache-disk gave the video composites of slow moments a disk tier, so a heavy loop plays whole from
lap 0 after a restart. Its class walk left two other per-process results:

- **found-defects 771** — the audio's kept final mix (`MixStore`, cached-range-plays, the owner's "вариант
  А" of invariant 2) is the render thread's memory. After a restart every covered range is rendered
  live again: decode and every effect, each lap until it is kept again.
- **found-defects 770** — the Datamosh history (the stream memo) lives in one process. The owner decided
  (D6, answering the cache audit's cut 5): "Только финальные кадры". A moment whose final frame is on disk
  needs no history; any other moment walks its history from the cut again. The row is closed by that
  decision, not by a disk tier.

The owner's rule is "caches are allowed where they pay" (2026-11-01): keyed by what determines the result,
bounded, invisible to determinism.

## What Changes

- **One tenant of the project's cache folder** (`project-cache-folder`): `mix/v<FORMAT>/` for mix runs,
  written in the folder's cycle (a stale salt dropped on a miss, the lowest value per byte out, never what this
  process used in the last minute).
- **The kept mix pages to disk.** Kept bursts are written at rest as runs of raw f32 per context
  (linear play, each loop region's laps); after a restart the player's thread loads the runs ahead of the
  playhead and hands them to the render thread, which stays the store's only owner (invariant 2) and
  never touches the disk.
- **Keys add what a process could assume**: the identities of the audio files heard (path, size, mtime) and
  the store's salt (`project-cache-folder` D1b: the decoder's major and the versions of the kernels that ran;
  not the whole build).
- **The restart line**: the trace states what a restart cost (history positions walked from the cut, audio
  renders of covered bursts) and what the mix tier restored. The history half is the price of the owner's D6,
  shown, not stored.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `audio-render`: the kept final mix has a durable tier, loaded off the render thread.
- `telemetry`: the restart's cost and the mix tier's restores and writes in the trace.

## Impact

- `src/libs/kt-engine/src/mix.rs` (`MixStore` restore handoff, `Unit.data` an `Arc`), `player.rs` (writes at
  rest by the prerender's quiet rule, loads ahead of the playhead, a second `rtrb` ring into the render thread),
  `audio.rs` (the file identities of a burst's widened range), kt-cli `trace summary` and `trace cache`.
- Invariants: 2 (the render thread still owns the mix alone; the disk is the player's thread's), 4 (the bars: a
  burst on disk counts as covered only when restorable), 11 (the export never reads the mix: the audio export
  stays a live render, byte for byte).
- No project format change; no new dependency (`lz4_flex` was the history tier's and goes with it).
  `KT_RENDER_CACHE=off` turns the tier off with the composites.

## The owner's decisions

- **A14 (kkt-26.11.20 line 19), Q1 b — build now;** "у нас … будет подпапка в папке проекта с разными кешами":
  the mix runs are a tenant of the project's cache folder.
- **D6 (kkt-26.11.22 line 9) — "Только финальные кадры":** no history on disk. It replaces A14's "build now" and
  Q2 b's LZ4 for the history half. His question about compressing the history (D9 of the previous revision)
  fell with it.
