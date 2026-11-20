# Proposal: magic-wand-mask

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3535–t3544.

## Why

The owner, `kkt-26.11.01` G1 (line 71; registry `kkt-26.11.01-index.md`): "Крутой механизм
magic-wand-масок". Classic masks are done (shape-masks: rectangle, ellipse, Bézier pen, feather, modes,
on the picture, a slot and a track). A wand picks a region by clicking on the picture. Note F-MSK-008
(new) found that video editors either key a colour range without contiguity or segment objects with a
neural model; a contiguous wand holds in video only when its seed follows the object — which the point
tracker (change `point-tracker`) provides.

## What Changes

- **A wand mask shape** beside rectangle, ellipse and Bézier: seeds (normalized points, each add or
  subtract), a tolerance, contiguous or colour range, the colour space (RGB, luma, chroma), anti-aliasing
  from the colour distance, grow/shrink and fill holes; it joins the mask list with the existing modes,
  feather, opacity and invert, on the picture or an effect slot (a track wand is refused with its reason in
  this stage: the track's composite is not on the CPU).
- **Recomputed per frame from the source picture**: the coverage is a pure function of the event's decoded
  frame, the seeds at that frame and the parameters, computed by the one mask rasterizer at an analysis
  size and memoized by those inputs.
- **Seeds can move**: a seed's x/y are ordinary mask rows, so they take keyframes, formulas and links — a
  seed linked to a tracker point follows the object.
- **Clicks on the preview**: right click → Mask → Magic wand; click adds a seed, Shift-click adds, Alt-click
  subtracts; the tolerance is dragged on the preview (drag from the seed) and shown live on the shown
  frame; the matte view (the existing mask overlay) shows the result.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the wand shape in the mask rasterizer; its inputs, determinism, the memo.
- `project-format`: a wand mask raises the file by content.
- `ui-reach`: the wand on the preview's mask menu, seed clicks, tolerance drag.

## Impact

- kt-model: `MaskShape::Wand { seeds, tolerance, contiguous, space, soft, grow, fill_holes }`, seed rows
  `s<id>.x/.y` (stable ids, as vertices), validation.
- kt-fx: `mask::wand_coverage(frame, w, h, shape)` — a scanline flood fill / colour range, distance
  ramp, morphology; integer and IEEE-exact operations.
- kt-engine: the layer's mask plan hands the source frame bytes (already in the cache) to the rasterizer
  for a wand; the coverage memo keyed by (content serial, the wand's resolved values, size).
- kt-app: preview gestures, i18n ×5.
- Format: next free `.kt` version by content; OTIO row (masks are already metadata-only).
- Idle: a paused frame's coverage is computed once; at rest nothing runs.
- Feature note: F-MSK-008.

## Questions to the owner

**Q1. An algorithm now, a neural object mask later?** Your words: "Крутой механизм magic-wand-масок".
Your decision for the professional keyer (2026-11-01): "an algorithm, not a neural network, as the base …
the same bytes on every machine and no ML runtime or model weights in the program. A neural matte may come
later as an extra assist." Resolve's Magic Mask, AE's Roto Brush and FCP's Magnetic Mask are neural.
- (a) **The algorithmic wand (contiguous or colour range, seeds that can follow a track) now; a neural
  object mask later as a separate assist** (*recommended*, the keyer's rule).
- (b) A neural object segmentation now, downloaded on a click like the speech recognizer.

**Q2. What does a wand on an effect slot read?** Today a slot mask mixes the slot's result with its dry
side; the wand needs pixels to choose a region.
- (a) **The event's SOURCE picture (as decoded, before effects), on the CPU** (*recommended*: no GPU
  readback; "select the sky, grade it" is the use).
- (b) The picture at the slot's input (after the previous effects): exact but needs the intermediate frame
  read back from the device per frame.
