# Proposal: ui-frame-cost

## Why

The owner, 2026-11-07, on build kt26.11.07-61414dba with his own project (`docs/ktprompts/kkt-26.11.07b.txt`,
item 1, registry row G1): "движение мышкой на паузе стоящей на событии при движении мышки по превью - в htop
показывает cpu 25%", and "это не prefetch". In his `load-report.txt` the UI thread holds a whole core
whenever it draws: during playback 45–50 UI frames a second saturate it, and with the effects window
open it draws two frames a second at 100 %, half a second a frame. Earlier (`kkt-26.11.07` E9) he said the
cost of a mouse move depends on the timeline's scroll position.

The mechanism: the work of one UI frame grew with the size of the PROJECT, not with what is on screen.
A frame copied the whole document several times, walked all 1974 events for things it then culled,
and laid a speed curve out again per pixel column.

## What Changes

- The model lends the document instead of copying it per question (`TimelineModel::doc`/`doc_arc`, one
  per state); every drawing path that asked `document()` per frame reads it.
- A per-state event index (`EventIndex` in `TlFacts`): the event pass, the coverage strip, the hatching
  of a tail past the material and the hit test under the pointer ask it what meets the view.
- `ClipWarp` lays its speed integral out once (bit-identical to `source_progress`).
- The media bin is a virtual grid: rectangles by arithmetic, only visible cards drawn; its card list and
  new-card placement are kept per state.
- The coverage poll asks the events in view; the decode qualities are grouped once per poll.
- The UI frame's own processor time is measured by stage (`frame_cost`) and shown in Diagnostics →
  Performance, in the load report and in the bench dump.
- Tests t1910–t1914, the live scenario `ui_frame`.

## Impact

- `kt-model` (model, warp), `kt-engine` (video.rs reads the laid-out integral), `kt-app`.
- No format change, no new dependency. Invariant 11 untouched: the integral reads the same bits (t1910),
  and the frame meter steers nothing.
