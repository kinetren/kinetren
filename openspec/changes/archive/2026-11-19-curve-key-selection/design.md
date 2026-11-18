# Design

## D1. One selection, keys derived from segments

`SegSel` keeps segments by their left key's tick (segment-shape-fields D4). It gains two tick lists:
`keys` (taken in by Shift+click) and `off` (a segment end taken out by Shift+click). A key is selected
when it ends a selected segment or is in `keys`, and is not in `off`. Ticks, not indices: an index
shifts when a key is inserted before it. Derived, not copied: the segment rows, the shape menu and
the key drag all read the same selection, and there is nothing to keep in step (his answer п.1: "the
keys of the selected segments").

Rejected: a separate key set beside the segment set (his option б) — two selections that disagree is
the Resolve trap the F-AUT-005 note records (Keyframe Editor vs Curve Editor).

## D2. The joint drag

At the press the curve's keys are copied once (`KeyDrag.orig`) and the selection with them; every
frame of the drag writes `orig` shifted, so the shift never accumulates rounding. The grabbed key
follows the pointer and snaps to the frame grid as a single key does; `dt` is its shift. `dt` is
clamped so no selected key reaches an unselected neighbour (the order never changes, so no re-sort and
no two keys on one tick — trap 3 of F-AUT-005) and the block does not leave the visible axis (a block
already partly outside may still move inward). The value shift is taken on the graph's scale
(`Axis::map`), with the value window frozen for the gesture, so a logarithmic parameter moves by the
same visual distance; each value is clamped to its passport. The selection's ticks move by `dt` with
the keys. The carrier coalesces the gesture's edits into one history step, as for one key.

## D3. Modifiers

Shift+click on a key toggles it (before: it selected the key as a plain click). Shift on a sweep adds
(before: only Ctrl). Shift+click on a segment keeps its run behaviour. A plain press on a key outside
the selection drops it, as a plain click on empty space already did; a press on a selected key keeps
it, or the drag could not start.

## Class walk

The mechanism: one point moved at a time where a person wants a group. Checked:
- every curve carrier — the effect slot, the generator, a composition parameter, the camera, the
  transition, a mask's curve: all draw `automation::draw`, so all get the selection (one widget, item
  221);
- the curve table (`automation::table`): a row per key, edited by number; a group move there is a
  typed shift, not asked for — noted, not done;
- mask vertices (`maskctl.rs`): one selected vertex; moving several contour points is a different
  request, not asked for — noted;
- timeline events already have a set selection and a batch move (items 202–205); markers and the
  level line have no group gesture asked for.

## D4. A key drag keeps its zone (found on the way)

The graph decided "a gesture on the ruler" by the pointer's CURRENT height: a key at the top of the
value window pulled upward crossed into the ruler strip, the key stopped and the main playhead jumped.
The zone is now decided by the press point, as the shape handle's grab already was. t3114 drags the
top key up past the plot.

The same mechanism had a twin: `hit` (the key under the pointer) was read at the CURRENT position on
the frame egui declares the drag, which is past its threshold — a quick pull left the key's grab
before the drag began, and the gesture became a sweep over empty space. A drag's key is now taken at
the press point too. Both are the class "a gesture's target read where the pointer IS, not where it
was pressed"; the other gestures of the graph (the shape handle, the sweep's start, the menu target)
already read the press point.
