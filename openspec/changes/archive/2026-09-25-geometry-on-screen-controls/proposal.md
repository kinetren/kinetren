# Proposal

## Why

The geometry of an event is the only node with which a person edits the FRAME, and today it can
be edited only with numbers in the FX window. The owner (R.63 item 6): "the geometry fx is still very poor,
solid boring input fields, even Vegas does it better… we must make it more convenient than anyone.
AT MINIMUM we should have editing right in the preview that is convenient". A clarification from the same
round: the on-screen controls are **in addition** to the fields, and the fields themselves are to be "in a normal form".

Why now: the parameters for the control are already fully ready (`EventGeometry` — crop, position,
zoom, rotation, anchor; all in FRACTIONS of the canvas, all are automated, all are published since R.63), and the
placement arithmetic is already computed by one function in `kt-fx::geom` — exactly the gesture
over the frame is missing. The reverse dependency is stronger than the direct one: without a single gesture over the frame, masks (F-MSK-001) and on-screen editing of titles (F-TXT-001) are blocked.

## What Changes

- **A transformation frame over the preview frame of the SELECTED video event**: a move by the body, eight scale handles, a rotation handle, the anchor point, four crop handles. The gesture edits the
  `EventGeometry` of that event which has the leading node — at the tempo of the gesture (the live value into the
  player every frame) and in ONE history step per gesture.
- **The anchor moves without moving the picture** (position compensation), and this is correct on a rotated
  layer too — the place where After Effects has a defect, confirmed by quotes in F-UI-001.
- **The placement rectangle of a layer becomes a publicly available pure function**: it is called by
  the geometry program (as now), by the interface (the drawing of the frame), and by the inverse recomputation "screen
  shift → parameter". A second placement arithmetic is not introduced (rule 17).
- **The display of the controls** — a toggle in the preview; during playback the controls hide
  themselves (the industry rule: FCP always hides, AE does so automatically), the cost of a frame without a gesture
  does not grow (invariant 12).
- **The form of the geometry fields in the FX window — a "Transformation" section** modelled on Premiere/AE:
  collapsible groups, X/Y pairs on one row, units (× / ° / %), sliders at zoom and
  rotation, a frame scheme next to the numbers, "reset geometry" in one action. The fields and the
  control are one and the same parameter and one and the same number (rule 17).
- **The fraction of the preview stops distorting the proportions of the frame**: today 1920×1080 at 1/8 gives
  240×134, at 1/16 — 120×66 (1.79 and 1.82 against 1.78), that is, "fit" on the preview and on
  the export compute DIFFERENT frames. The fraction chooses a size similar to the project frame.
- NOT included: cameras and `Pan3d` (their control is the next pass), masks, the keyframe path,
  guides (safe zones, grid), "before/after" comparison modes. The reason — each of them
  adds its own set over the same frame and its own fork "where are my handles".

## Capabilities

### New Capabilities

(none — all the work falls on the existing capabilities)

### Modified Capabilities

- `ui-timeline`: the transformation frame over the preview frame (gestures, hit zones, hiding during playback,
  frame cost), the form of the geometry fields "Transformation", the similarity of the preview fraction to the project frame.
- `video-plan`: the placement rectangle of a layer is given OUTWARD by the same pure function
  that builds the geometry program; the inverse recomputation "canvas point → node parameter" is its pair.
- `ui-reach`: the reachability of the new gesture and its toggle from the point where the person acts;
  a right click on the frame is not silent; a live scenario for each gesture.

## Impact

- `src/libs/kt-fx/src/geom.rs` — move the arithmetic of placing the leading node into a public
  function (`placement`) and its inverse pair; the body of the geometry program calls the same one.
- `src/apps/kt-app/src/app.rs` — `draw_preview`: the controls layer, hit testing, drags, hiding
  during playback; `preview_rect`/`img_rect` outward for the live scenarios (`KT_GEOM_DUMP`).
- `src/apps/kt-app/src/fx_editor.rs` + `widgets.rs` — the "Transformation" section of the leading node.
- `src/apps/kt-app/src/app.rs` — the choice of the preview fraction (similarity to the project frame).
- The project format does NOT change: the existing fields are edited by the existing commands
  (`Command::SetGeometry`), the state "show the controls" is the viewport (not the content).
- Tests: the harness `layout_tests.rs`/`reach_tests.rs`, pure functions in `kt-fx`, the live
  scenario `src/scripts/live/geomctl.py`.
