# Proposal: magic-wand-mask

> DESIGN ONLY, revision 2 (2026-11-24, base `0c2a4065`): made implementation-ready against today's code; the coverage
> is a contour now, not a raster (design D0). Tests: t3535–t3544. No open question.

## Why

The owner's items, whole:

- `kkt-26.11.01.txt` lines 68–73 (G1): "Следующий запрос: ``` Крутой механизм magic-wand-масок ```"
- `kkt-26.11.20.txt` line 28 (A20), the one answer to Q32–Q35 (tracker first or plane, where its controls live, an
  algorithm or a neural wand, what a wand on an effect reads): "первые два, по умолчанию - самые удобные и точные. позже
  сеть - нейросеть"

**The reading, checked.** "Позже сеть" decides Q34: the algorithm now, a neural mask later. "По умолчанию — самые удобные и
точные" is a rule for the defaults of both tools. Q35 (the source picture or the slot's input) is named by no word
of his; the registry took the recommended one, and this design keeps it, with what it costs said in D3. His rule of
exactness decides three things here: the region is cut at a sub-pixel contour, it is computed per frame and not
carried over, and it reads the decoded source whatever the preview's quality would need.

Classic masks are built (shape-masks). F-MSK-008: editors key a colour range without contiguity or segment with a
network; a contiguous wand holds in motion only when its seed follows the object (keys now, `point-tracker` later).

## What Changes

- **A wand shape** beside rectangle, ellipse and path: seeds (add or subtract), tolerance, contiguous or every matching
  pixel, the colour space, fill holes. It lives in the same mask lists (picture, effect slot) with the same mode, invert,
  opacity, feather, falloff and expansion.
- **Its outline is computed per frame** from the event's decoded source and the seeds at that frame, as closed contours;
  everything after is the one mask rasterizer.
- **Seeds are mask rows**: keys, formulas and links as any vertex has.
- **On the preview**: right click → Mask → Magic wand; a click adds a seed, Ctrl+click a subtract seed, the ring
  around a seed sets the tolerance, the outline follows on the paused frame.
- **Refused or skipped, said**: a track (no picture on the processor), a composition instance (the same).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the wand shape in the rasterizer, its inputs, determinism, the memos.
- `project-format`: a wand raises the file by content.
- `ui-reach`: the wand in the mask menu and list, its gestures.

## Impact

- kt-model `mask.rs`: `MaskShape::Wand`, its rows, `check_masks`; a wand on a track refused.
- kt-fx `mask.rs`: contours of a mask (one or many), `wand.rs`: box mean, distance, fill, marching squares; `MaskOp.pixels`.
- kt-engine `video.rs`: the order of the source frame for a stream event, the pixels in `layer_src` and in the stream's
  inputs.
- kt-app `maskctl.rs`, `masks.rs`, `app.rs` (menu): the tool, gestures, list row; i18n ×5.
- Format: next free version by content. OTIO: masks are metadata only (planned row there). Idle: nothing at rest.

## Questions to the owner

None. Q34–Q35 as asked are in the git history of this file (ffe4eebf).
