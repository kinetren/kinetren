# Design

## D1. The selected slot's centre, in its own frame

The crosshair follows the selection of an effects window (`ChainSel::Fx(i)`, a raw slot index): one
effect's point at a time, as the masks edit one owner. The point is −1..1 across the frame the kernel
sees (`radial_blur.wgsl`: `cx = 0.5 + center_x / 2` of the pass's width). A slot before the Geometry
node works on the source, which the node places on the canvas; after it, on the canvas
(`ClipEvent::geometry_pos`). `maskctl::slot_frame_view` gives that map — the one the slot's masks use
(placed or plain `MaskSpace`), so the crosshair lands where the rays spread from.

## D2. One step, the field's rules

A drag takes a gesture token at its start and writes through `apply_coalesced`: one undo step. Only a
real change is written (input-without-work). A pair with a live curve gets a key at the playhead's
material moment (`clip_material_at`, `insert_key`) through `SetFxAuto`, as the field edits the curve
it shows (R.43 item 291); the base stays.

## D3. Where it is drawn

From the one place in `draw_preview` where the R.64 frame and the masks are, after the frame: egui
gives the pointer to the last registered interactor, so the crosshair wins over the frame's body. Not
while masks are edited (they own the picture) and not during playback (the frame's rule).

## D4. The pair on one row

`params_grid_bound` laid parameters two a row by position: Radial Blur's Amount took Center X's row
and Center Y fell to the next. An `_x` followed by its `_y` is now one row of its own; the rest stays
two a row. The node inside the chain strip is `fx-graph`'s and is not touched.

## Class walk

The mechanism: a point parameter set only by numbers, its halves apart. Checked:
- every effect passport: only Radial Blur has a centre pair today; Twirl, Lens Distort, Ripple,
  Kaleidoscope and Vignette fix their centre at the frame's middle in the kernel — a centre for them is
  a kernel and passport change (noted, not done);
- Drop Shadow's `offset_x`/`offset_y` (px) — a vector, not a point on the frame; its pair is now one
  row; dragging it on the preview is not asked for;
- generators' `plate_pad_x/_y`, `offset_x/_y`: one row by the same grid rule where the grid draws them;
- the Geometry node's position, anchor and rotation already have the R.64 frame;
- masks have their own on-preview editor (shape-masks).
