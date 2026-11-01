# Design: segment-shape-fields

## Context

The segment shape lives on the LEFT key of each segment (R.61, `SegShape` in
`src/libs/kt-model/src/project.rs`); the model needs nothing. The widget (`src/apps/kt-app/src/automation.rs`)
already knows one "selected segment" — `AutoView::sel_seg`, set by the right click, used only to show
the segment formula field. The parameter fields exist only in `shape_all_control`
(`src/apps/kt-app/src/widgets.rs`), which reads the FIRST key's shape and writes its edited copy into
every key. The R.67 handle (`shape_dragged`) has its own bounds (tension 0…4, periods 0.25…256) — the
same numbers typed a second time in the "all" row.

## Goals / Non-Goals

Goals: fields for one segment and for a selection, mixed display, one command per edit, gesture-only
writes, keyboard reach; the "all" row unchanged in look. Non-goals: segment selection across curves,
selection in the table view, a track-level automation lane, changes of undo granularity of the FX
window (t193: a window session is one history step — unchanged).

## Decisions

### D1. One control over a set of segments
`shape_all_control` becomes `shape_fields(ui, salt, label, curve, targets: &[usize], hover) -> bool`:
`targets` are left-key indices. The "all" row passes every segment (and, as before, writes the last
key too, so the curve stays uniform); the selection row passes the selection. One body — "paired
ways of doing one action, one body" (ui-reach). The R.67 handle and the fields read the parameter
bounds from ONE table (`widgets::TENSION_BOUNDS`, `PERIODS_BOUNDS`, `AMP_BOUNDS`) instead of two literals.

Revision (of the "all" row's display): it showed the first key's shape even when segments differed.
It now shows the mixed mark. Its layout, width, label and hint are untouched (the look the owner saw).

### D2. Mixed values
Shape kind compared by discriminant over the targets; a parameter compared by value across targets of
that kind. Mixed → the field's text is "—" (a custom formatter), the shape combo's text is "—", and no
parameter fields while the kinds differ (tension of a smooth and periods of a zigzag have no common
meaning). Edits:
- typed number (focus + change) → absolute on every target;
- drag → the delta of this frame added to each target, each clamped to the parameter's bounds — the
  differences between the segments survive (the common behaviour for several selected keyframes in
  AE/Resolve; setting all to the first one's value on a drag would destroy the work);
- shape pick → targets of another kind take the picked shape's defaults, targets of the same kind
  keep their parameters (picking "Zigzag" on a selection must not reset the zigzags already tuned).

### D3. Writes only from a gesture
The field is a `DragValue` with `clamp_existing_to_range(false)`; the result is written only on
`dragged()` or `has_focus() && changed()` (and on a list pick). Same rule as R.67 t786 for keys: drawing
must not edit. The old "all" row clamped periods > 256 from the file each frame — t1120 is red on it.

### D4. Selection state keyed by the left key's TIME
`AutoView::seg_sel: SegSel { ci, left: Vec<Tick> }` replaces `sel_seg: Option<(usize, usize)>`.
Indices shift when a key is inserted before a segment; the left key's tick does not (a key is
inserted at a new tick; a split keeps the left part selected, which is the segment the person
pointed at). Resolution to indices happens once per frame (`SegSel::indices`), a tick without a key
or without a right neighbour drops out silently. Keys dragged to a new time leave the selection —
acceptable: the person is then working on keys, not segments. The single-segment formula field (R.67)
reads "exactly one selected segment with a formula shape".

### D5. Gestures
- right click: on a segment in the selection — the menu acts on the selection (header with the
  count); otherwise selects that segment alone (former behaviour).
- Ctrl+click on a segment (no key, no handle): toggle; Shift+click: the run from the anchor.
- drag starting on empty space (not key, not handle, not ruler) — today it does nothing: it becomes
  a sweep that selects the touched segments, drawn as a translucent band; Ctrl adds.
- plain click on empty space: places the key (unchanged) and clears the selection.
- switching the active curve clears it (the selection belongs to one curve).
Ctrl is not taken elsewhere on the graph (the wheel's Ctrl is zoom — a different event); Shift on
the handle drag stays "periods" (the handle is hit first).

### D6. Where the row stands
Directly under the "Shape of all segments" row in `curve_block`, inside the window's scroll body (R.66
form contract: numbers in the body, actions outside — the row has no window actions). The caption is
`Selected segment {0}–{1} s` or `Selected segments: {0}` (no number agreement, i18n rule 2); a small
"×" button at the end clears the selection.

## Risks / Trade-offs

- Mixed "—" in a `DragValue`: on focus the text "—" is selected, typing replaces it. Arrow keys while
  focused act as an absolute set from the first target's value — acceptable, documented here.
- Sweep vs click: egui separates them by its drag threshold; a tiny drag selects nothing and is not a
  click — the same as today (no key).

## Migration

None: no model, file or OTIO change.
