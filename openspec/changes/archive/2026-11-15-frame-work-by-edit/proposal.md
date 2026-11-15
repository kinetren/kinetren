# Proposal: frame-work-by-edit

## Why

`docs/found-defects.md` holds thirteen open rows with one mechanism: work done per frame or per edit
is keyed by the whole project (a scan of every event, every reel, every placement) instead of by an
index or a revision, so its cost grows with the size of the project and not with what the frame
shows or what the edit changed. The rows: 118, 134, 153, 154, 156, 157, 168, 169, 190, 191, 192, 364,
365. Each is small on our showcase and on the owner's 1974-event project; together they are why a
frame on a long edit costs more than the same frame on a short one, and why a value edit of a title
costs reels it does not touch.

The orchestrator's brief (2026-11-15): close these rows as one change; the mechanism is one, walk
every subsystem where it can act the same way.

## What Changes

- **A time index of a renderer's layers** (`ClipCover`): the events active at a tick are found by
  halving over boundary runs laid out once per build, in the composite order. The plan, the order
  (`layers_at`, `wanted_frames`), the stream stages' advance and want, `stream_frame_bytes` and the
  coverage of a reel ask it; a transition's partner and a track stage's events and revision are laid
  out once per build. A frame costs the events active at it (row 153).
- **The reel coverage bars cost the window** (row 154): a reel's coverage over an instance's window
  asks only the inner events that window covers (the index), the instances of one reel with the same
  window and quality share one answer per call, and the cold parts are subtracted in one sorted pass.
- **kt-model lookups by index** (rows 156, 157): `Project::sequence` answers by an id index kept with
  the reels (rebuilt when the list changes), a staircase transition's third-event check asks only the
  events of its lane near the window.
- **A frame resolves its curves without a whole-chain copy per plan** (rows 168, 169): a layer's
  applied chain is memoized per (layer, material tick) on the renderer, shared by every plan of that
  moment (the probe, the composite, a retry, the loop fill); transitions, generators, cameras and masks
  set their curves by compiled index.
- **A title keystroke resolves only what it can reach** (row 190): the other addresses of an edited
  generator are resynced by the builder thread, not by a whole `gen_sources` on the UI thread.
- **An export asks the build it already has** (row 191): the serial/pipelined choice reads the
  renderer each path builds, never a throw-away one.
- **An edit's build hashes what it reads once** (row 192): the event's derive key is computed once per
  build and shared by the audio and video builders; the remaining whole-project walk is measured on a
  ×10 synthetic project against the edit budget.
- **The executor and the simulations keep their per-frame vectors** (rows 364, 365).
- **GPU submits and the first loop lap** (rows 134, 118): measured on the showcase; the per-frame part
  that grows with the project goes, the rest is assigned with numbers.
- **A counting guard** (`kt_time::cost` rows `FrameClip`, `ChainClone`, `ParamByName`): per-frame and
  per-edit work at ×1, ×4, ×16 events must not grow with events outside the frame.

## Capabilities

- `video-plan`: a frame's work grows with the events active at it.
- `compositions`: reel coverage grows with the placements' windows.
- `automation`: one resolved chain per (layer, moment); carriers by index.
- `generators`: a value edit costs its own addresses on the UI thread.
- `export`: no throw-away renderer.
- `transitions`, `project-format`: lookups by index.
- `time-model`: new cost rows.

## Impact

`kt-time` (cost rows), `kt-model` (sequence index, staircase check, carriers' compiled curves),
`kt-engine` (video index, coverage, chain memo, export, player gen resync, builders), `kt-gpu`
(executor allocations), `src/plugins/fx-video-*` (simulation plans), tests t2660–t2679.
