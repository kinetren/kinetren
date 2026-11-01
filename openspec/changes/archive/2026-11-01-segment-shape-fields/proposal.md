# Proposal: segment-shape-fields

## Why

The owner, kkt-26.11.01 item 10 (verbatim in `docs/ktprompts/kkt-26.11.01.txt`, line 19; registry
row K10): the automation curves on a particular region give him no place to set their parameters —
"I want to set the zigzag's parameters and there is nowhere to write them; if I set the zigzag for the
whole curve, then there is… and what if I have 100 segments with parameters? You cannot write them
all in one place like that."

Today the curve block under the graph has fields for the shape parameters only as "Shape of all
segments" (`src/apps/kt-app/src/widgets.rs` `shape_all_control`, called from
`src/apps/kt-app/src/automation.rs` `curve_block`). One segment has only a drag handle at its middle
(R.67: vertical drag → tension/amplitude, Shift+horizontal → periods), with no number and no profile;
several segments cannot be selected at all. So a zigzag on one region can be tuned only blindly by the
mouse, and a hundred regions only one by one.

The "all" row also has a latent defect of the class R.67 already named ("a key only from a gesture"):
its numbers are `DragValue`s with a range that clamp an out-of-range value from the file (the model
takes periods up to 1024, the field 0.25…256) and report "changed" every frame — a silent write with
the window merely open.

## What Changes

- The automation graph gets a **segment selection** on the active curve: a right click on a segment
  selects it (as today), Ctrl+click toggles a segment, Shift+click extends from the last one, a drag on
  an empty spot of the graph sweeps the segments it covers. Selected segments are tinted on the graph.
  A plain click on an empty spot still places a key and clears the selection.
- The curve block gets a row **"Selected segment …" / "Selected segments: N"** right under the
  existing "Shape of all segments" row: the same shape combo and the same parameter fields (tension;
  periods, amplitude, profile), showing the segment's own values. For a selection a field whose
  values differ shows "—" (mixed); typing a number sets it on every selected segment, dragging the
  field shifts every selected segment by the same amount (each kept in its range); picking a shape
  sets it on every selected segment and keeps the parameters of those already of that shape. A clear
  button drops the selection. The row is keyboard-reachable (Tab, typing, Enter).
- A right-click shape choice on a segment that belongs to the selection applies to the whole
  selection, and the menu header says how many segments it touches.
- Both rows are drawn by ONE shared control (the "all" row becomes this control over all segments):
  the "all" row keeps its look, but now also shows "—" when the segments differ instead of silently
  showing the first one's shape.
- The shape fields write ONLY on a human gesture (drag, focus+edit, a pick): an out-of-range value from
  the file is shown, not clamped into the model.
- Every selection edit is ONE command for all N segments; inside the FX window it joins the window's
  undo step like every other edit there (t193, unchanged).

No model or file-format change: the shape already lives on each key (R.61). Nothing new for the OTIO
export to lose.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `automation`: a new requirement — the shape parameters of ONE segment and of a SELECTION of segments
  are edited by fields next to the "all segments" row, mixed values shown as mixed, one command per
  edit, writes only by gesture.
- `ui-reach`: a new requirement — selecting segments is reachable by mouse and keyboard and is said
  (graph hint, menu header, the row's caption).

## Impact

- `src/apps/kt-app/src/automation.rs` — segment selection state in `AutoView` (replaces `sel_seg`),
  the selection gestures and the tint on the graph, the menu applying to the selection, the new row in
  `curve_block`.
- `src/apps/kt-app/src/widgets.rs` — `shape_all_control` becomes a shared `shape_fields` control over a
  set of segments with mixed display and gesture-only writes; one table of shape-parameter bounds used
  by the fields and the handle drag.
- `src/apps/kt-app/i18n/*.toml` — new strings in all five catalogs.
- Tests t1115–t1124 in `src/apps/kt-app/src/layout_tests.rs` / `reach_tests.rs`; live scenario
  `src/scripts/live/segfields.py`.
- `docs/guide/whats-new.md`, registry row K10.
