# Proposal: input-without-work

## Why

The owner, 2026-11-07, on build kt26.11.07-61414dba (`docs/ktprompts/kkt-26.11.07b.txt`, rows G5 and G7
of its registry): with an event's effects window open, every mouse move made prefetch take a whole
core. With VHS Tape and Camera ISP switched off on video and audio it went on; with another effect
selected in the chain it stopped. "ТРЕБУЮ ПОЛНУЮ РЕВИЗИЮ!" His load report shows prefetch ~90 % and
audio-render ~85 % for 20 s with no audio rendered, and two lines read "at rest" at 196 %.

The mechanism, not the panel: input that changes nothing did work. A number field clamped a stored
value on display, reported "changed" without a gesture, the window sent the value back, the model took
an equal value as an edit, and every edit handed the player a whole new project, which rebuilt the audio
renderer on the audio-render thread and both renderers on the prefetch thread. None of that had a work
counter, so the load watch called it rest.

## What Changes

- A number field writes only from a human gesture, and a value its carrier already holds is no edit
  (`widgets::param_edit`, `param_row_bound_ex`, `event_gain_field`).
- The model takes a command or a batch whose result equals the canon as no edit: no undo step, no
  revision (`TimelineModel::revision`); the app's edit doors then bump nothing, hand the player nothing
  and mark nothing dirty.
- The other places of the class found by the walk (design D3): the live level drag hands the player a
  project only when the level moves; the effect colour buttons round instead of truncating and write
  only a changed colour; a thumbnail that fails to decode is a failure, not a re-order per frame; the
  speed drag sets the rate only when it changes.
- The UI's own audio renderer (the pause meters, the loop's pinned head) is built once per project
  version: a loop region change built a whole one on the UI thread.
- Work counters `model_edits`, `project_swaps`, `engine_builds`; the load watch calls a busy thread with
  no counter moving "NO COUNTER MOVED", never "at rest"; Diagnostics says so.
- The live scenario `idle` selects the VHS Tape device panel, drags a number to its floor and moves the
  pointer: the engine threads' processor time and the new counters must stay at zero.

## Impact

- `kt-time` (work rows), `kt-model` (`revision`, `apply_coalesced` returns whether it changed),
  `kt-engine` (counters, the shared UI renderer), `kt-app` (widgets, edit doors, timeline, effect colours,
  thumbnails, speed, load watch, Diagnostics, five catalogs, the CJK subset), `scripts/live/idle.py`.
- No file format change. The report's line format stays (its rework is `telemetry-journal`'s); only the
  verdict word changes when a thread is busy with no counter.
