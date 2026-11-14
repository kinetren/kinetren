# Proposal: scheduler-followups

## Why

The scheduler's three stages (`one-scheduler`, `gpu-arbiter`, `memory-ledger`) left four places
where the interface or the preview still waits on another thread's whole job
(`docs/found-defects.md`, the owner's standing go of 2026-11-09 to take the audit table's rows
as slots free):

- **270:** opening a project stands on the UI thread. The new app's constructor opens every
  source (FFmpeg probe and sidecar) and builds the player's first version. Replacing the app then
  joins the old player's threads and decoders. On the heavy showcase the window stands for seconds.
- **252:** Ctrl+S held the UI for 2–3.6 s on a busy disk. This was found on a base from before the
  save lane existed; it needs to be proved on the current tree.
- **251:** a slow display never covers a loop. Only the moments it shows are kept, so each lap
  composes "for the first time" a different subset.
- **290:** the offline engine's `inner` mutex is held for a whole frame. The slow-route preview
  frame waits there behind an effect cell, a reel strip or a poster, not on the ring that bounds
  it.

## What Changes

- **Project open never stands** (270):
  - The project file is read and its lock taken on the scheduler's `media` lane (class `answer`).
  - The player is created with no sources and builds its first version on its own
    `engine-build` thread.
  - The sources open on the `media` lane and join the player as they open; the media lengths and
    the missing-file notices land when they answer.
  - The closing project's player and sound output are released on a thread of their own.
  - The window and the timeline show the new project at once; the preview says Loading until its
    build and frames arrive.
  - Undo, Save and edits act on the model from the first frame. An edit made during the load is
    built after the load's version, and a source changed during the load is swapped after its first
    open.
- **Ctrl+S under a busy disk** (252): proved on the current tree by the blocking counter while
  another writer saturates the same disk; nothing is left to fix if it stays at zero.
- **A covered loop becomes all-kept on a slow display** (251):
  - A loop-fill service of the scheduler (class `near`) runs while a loop plays. It composes,
    on an engine of its own, the loop moments that are not kept and whose frames are already
    decoded, and keeps them on the display's executor as the slow route does.
  - The arbiter admits its pieces behind the display's frames.
  - It stops when the loop is all-kept, when the loop or the play stops, or on an edit.
  - `KT_LOOP_FILL=off` gives the old behaviour for A/B.
- **The slow-route frame never waits behind side work** (290): effect cells, reel strips and
  posters move to a second engine on the offline device (`kt_gpu::side_engine`). It shares the
  device's compiled pipelines but not the frame's arena, pool and lock. The slow route keeps
  `offline_engine` to itself, and its wait for another frame is bounded by the ring, not by a
  mutex. The wait is measured in the trace (`FrameWait` lock).
- **The class walk**: every place where the UI thread or the preview's compose thread can wait on
  a lock, a file, a build or another class's whole job is listed in design.md with its verdict.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `durability`: opening a project (menu, recent card, start tile, drop, import, live rig) makes no
  blocking call on the UI thread; a save on a busy disk blocks nothing.
- `video-plan`: a loop that plays has its unkept moments composed in the background below the
  display's class; the slow-route frame has an engine no side work locks.

## Impact

- `src/apps/kt-app/src/app.rs`:
  - `build_player`, `open_loaded`, `new_with_project`, `replace_with`, the open call sites, and
    `take_media` with a new `MediaThen::Load`;
  - a new `opening` field with its take in the frame;
  - `fx_thumbs.rs`, `reel_preview.rs` and the posters move to `side_engine`.
- `src/libs/kt-engine/src/player.rs`:
  - `Player::new_deferred`, whose first build runs on `engine-build`;
  - the loop-fill service (`loop_fill.rs`) and `Player::set_fill_gpu`.
- `src/libs/kt-gpu/src/lib.rs`: `side_engine()` on the offline engine's context. `engine.rs`: an
  engine built on a device that already has kept pipelines takes them.
- `src/libs/kt-time/src/trace.rs`: a job kind for the loop fill and a block kind for the project
  file.
- `kt-cli preview`: injects the fill engine (`--no-fill` / `KT_LOOP_FILL=off` for the "before").
- Tests t2330–t2349; the live scenario `open_heavy` with a PATH line.
- No `.kt` format change, no plugin API change, no new dependency.
