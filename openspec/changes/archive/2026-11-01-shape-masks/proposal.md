# Proposal: shape-masks

## Why

The owner, kkt-26.11.01 line 66 (registry row S1): "a convenient mechanism of classic masks". The
same batch, line 55 (multicam): "do not throw features anywhere, as agents usually do — I forbid
it" — a mask must live where a person who comes from After Effects / Premiere / Resolve / Vegas
looks for it, not in a new panel. The program has no masks at all today: no geometry in the
model, no rasterizer, no on-screen contour editing. The reconnaissance is done — F-MSK-001 (the
shapes, the modes, animation by point correspondence) and F-MSK-002 (where a mask lives: on the
layer, on the effect, as an object) — and the R.64 transform frame gave us the on-screen gesture
machinery and the one placement arithmetic shared with the render. This change is also the base
for the later magic-wand masks (G1) and the point tracker (T1): a tracker will drive mask points,
so every point has a stable address that a link can name.

## What Changes

- **A mask in the model** (`kt-model::mask`): a shape — rectangle (centre, size, rotation,
  corner roundness), ellipse (centre, size, rotation) or a closed Bézier path (vertices with
  stable ids and two tangent handles each) — plus feather (px) with a falloff (smooth / linear),
  expansion (px, signed), opacity, invert and a mode (add, subtract, intersect, difference). An
  ORDERED list of masks combines top to bottom. Coordinates are normalized to the frame the mask
  belongs to, so the preview at ¼ and the export at 1/1 cover the same region.
- **Three owners, one list type:**
  - the EVENT's picture — `ClipEvent.masks`, applied inside the "Geometry" node in the order the
    node already reserved: crop → mask → placement (the mask follows the event when it is moved,
    scaled or rotated);
  - an EFFECT slot — `ClipEvent.fx_masks` (parallel to `fx`, like `fx_auto`): the slot's result is
    shown inside the masks and its input outside them ("limit an effect to a region");
  - a TRACK — `Track.masks`, applied to the track's composite after its chain (canvas space).
- **Animation through the one mechanism:** every number of a mask is a passport row (feather,
  expansion, opacity, centre/size/rotation, and per vertex `v<id>.x/.y/.ix/.iy/.ox/.oy`), its curves
  are `ParamCurve`s in the mask (the event's material domain), drawn by `automation::draw`; a key is
  written only by a gesture (R.67). A path is keyed AS A WHOLE: once it is animated, dragging any
  point at another time keys every point at that tick. A mask row is a link TARGET and a link
  SOURCE (`ParamOwner::Mask`, link text `mask<id>/…`, `fx<N>.mask<id>/…`), so the tracker can drive
  a point with a link. Track masks are static, refused loudly like every other track curve.
- **One raster path (invariant 7):** the mask list of an owner is rasterized ONCE per frame by one
  CPU function in kt-fx into an 8-bit coverage buffer: an exact-area scanline fill for the
  inside, a signed distance band around the contour for antialiasing, feather, falloff and
  expansion. The plan carries the masks as internal chain slots; the CPU applies the coverage
  directly, the GPU uploads THE SAME coverage bytes as a texture and applies it in one integer
  pass — conformance class EXACT. No masks → the plan, the bytes and the cost are as before.
- **Editing on the preview** (`kt-app::maskctl`): the R.64 interaction — handles over the picture,
  a grab zone wider than the drawing, one history step per drag, absolute recomputation from the
  press. Tools: Rectangle and Ellipse (drag to draw, Shift — square/circle, Alt — from the
  centre), Pen (click — a corner vertex, click-drag — a smooth one, click the first vertex or
  Enter — close, Esc — cancel the unfinished path), point editing (drag a vertex or a tangent,
  Alt+click a vertex — corner ↔ smooth, double-click a segment — insert a vertex, Delete — remove
  the selected vertex), a feather handle on the outer contour. Inserting a vertex into an animated
  path keys it at every key of the path so the shape at the keys does not change.
- **Where masks live in the interface (no new window, rule 22):** the event's masks — a "Masks"
  group of the Geometry node's form ("Transform", R.64) and the preview's right-click menu (Mask →
  Rectangle / Ellipse / Pen) for the selected event; an effect's masks — a "Masks" group under the
  slot's parameters in the effects window; a track's masks — a "Masks" section of the track window.
  ONE widget (`masks::mask_list`) draws all three.
- **Format:** the masks travel in the `.kt` project file; a file that has masks is written as
  format 2 BY CONTENT (an older build refuses it as "written by a newer Kinetren" instead of silently
  dropping the masks); a file without masks stays format 1 byte for byte. The OTIO door carries them
  in `metadata["kt"]` (envelope v14 by content); other programs see the unmasked picture — a row in
  "What OTIO export loses".

## Capabilities

### New Capabilities

(none — masks are part of the existing capabilities below)

### Modified Capabilities

- `video-plan`: masks in the plan (event / slot / track), one raster per frame, EXACT GPU class,
  "no masks — previous bytes", masks in the post-FX key.
- `video-fx`: the mask rasterizer (shapes, antialiasing, feather + falloff, expansion, modes,
  invert, opacity), resolution invariance, determinism.
- `automation`: mask rows in the passport, curves in the mask, path keyed as a whole, links to and
  from mask rows, track masks static.
- `project-format`: masks in `.kt` (format 2 by content), in the OTIO envelope (v14 by content).
- `ui-timeline`: the mask overlay and tools on the preview; the mask list widget.
- `ui-reach`: masks reachable where the person acts (preview menu, Geometry node, effect slot,
  track window), keyboard, texts in five catalogs.

## Impact

- kt-model: new `mask.rs`; `ClipEvent.masks/fx_masks`, `Track.masks`; commands `SetMasks`,
  `SetFxMasks`, `SetTrackMasks`; `SetFx` trims `fx_masks`; validation; links; clipboard blocks.
- kt-fx: new `mask.rs` (rasterizer + apply kernels); the planner knows the internal mask slots.
- kt-engine: materialization of masks into the chain, track stage, post-FX key.
- kt-gpu: `mask.wgsl`, `FxOp::MaskAlpha`, `FxOp::Mix` with a coverage texture.
- kt-otio: envelope v14 carries masks.
- kt-app: `maskctl.rs` (preview), `masks.rs` (the list widget), FX/track windows, i18n ×5, live
  scenario `masks.py`.
- Tests t1260–t1289.
