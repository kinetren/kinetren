---
id: F-MSK-008
title: Magic-wand mask — a region chosen by clicking on the picture
status: candidate
priority: B
cost: M
category: masks/shapes
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, photoshop-27]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-app]
invariants: [4, 7, 11, 12]
ui_rules: [13, 17, 22]
round: null
point: null
tests: []
depends_on: [F-MSK-001, F-MSK-005]
checked: 2026-11-20
---

# F-MSK-008 — Magic-wand mask — a region chosen by clicking on the picture

## Essence

The editor clicks on the sky, the wall or a shirt in the preview, and the mask takes the area of similar
colour around the click — no drawing of a contour. Shift-click adds, Alt-click removes, a tolerance says
how similar "similar" is. In video the question is what happens on the next frame: the area must be
found again, around a click that has to follow the object.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | AI Smart Masking (since 21) | an AI segmentation of people/objects into a mask, tracked; no colour wand [NOT VERIFIED live] |
| Adobe Premiere | 26 | Object mask (AI) | a click selects an object; the mask is propagated through the clip by a model [NOT VERIFIED live: helpx.adobe.com closed] |
| DaVinci Resolve | 21 | Magic Mask (Studio), Qualifier | Magic Mask: strokes on a person/object, a neural propagation forward/backward; the Qualifier is a colour range (HSL) with a picker, non-contiguous, the "wand" of colourists [NOT VERIFIED live] |
| Final Cut Pro | 12 | Magnetic Mask; Color Mask | Magnetic Mask: AI isolation of people/objects with analysis; Color Mask: an eyedropper colour range, non-contiguous [NOT VERIFIED live] |
| After Effects | 26 | Roto Brush 3 | strokes on a base frame, propagation by a model frame to frame, "Refine Edge" for hair [NOT VERIFIED live] |

**Outside the core.**

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Photoshop | 27 | Magic Wand, Quick Selection | tolerance, "Contiguous", "Anti-alias", "Sample all layers"; one still image |

## The trick behind it

**1. Video editors split the wand into two different tools, and neither is the Photoshop one.** A COLOUR
range without contiguity (Resolve's qualifier, FCP's Color Mask) is stable from frame to frame because it
has no seed — it is a key. A CONTIGUOUS region needs a seed, and a seed must move with the object; so the
"objects" tools (Magic Mask, Roto Brush, Magnetic Mask) all ship with propagation, and all of them are
neural. The plain Photoshop wand is missing in video precisely because a fixed click point falls off the
object on the next frames.

**2. The seed is the part worth tracking, not the region.** If the click point follows the object (a
point track), the contiguous region can be recomputed on every frame deterministically from the frame
itself — no model, no propagation drift, and an edit of the tolerance re-renders the whole clip at once.
That is the cheap, exact form of "an object mask" for objects that differ from their surroundings by
colour (sky, a wall, a shirt, a screen).

**3. The wand's edge is the quality.** A binary flood fill gives a stair-stepped, flickering edge; every
tool that people keep adds anti-aliasing from the colour distance (a soft ramp across the tolerance
boundary), feather, and grow/shrink — the same edge controls our shape masks already have.

## Pros

- No contour to draw; a mask of an irregular object in two clicks.
- Recomputed per frame from the frame: no drift, exact on every machine.
- With a tracked seed it follows the object without a neural model.

## Cons

- Fails where the object's colour touches the same colour in the background (the contiguous region leaks);
  needs a subtract click or a shape mask to fence it.
- Per-frame recomputation costs a pass over the frame on the CPU (at an analysis size).
- Without a tracked seed it holds only for static shots.

## What users say

NO QUOTES: no web access budget in this spec session (2026-11-20); the search would be r/davinciresolve
("magic mask" vs "qualifier"), r/premiere ("object mask"), the VEGAS forum ("smart masking"). The
editor rows carry [NOT VERIFIED] for the same reason.

## How it fits Kinotrain

### What already exists

- **Masks are one CPU coverage per raster**: `kt_fx::mask` (`src/libs/kt-fx/src/mask.rs`) turns an
  ordered mask list (add/subtract/intersect/difference, feather, expansion, opacity, invert) into 8-bit
  coverage that the GPU applies in one integer pass (`mask.wgsl`). A wand is one more mask shape whose
  coverage comes from pixels instead of a contour.
- **The pre-FX frame is CPU bytes in the frames cache** (`kt-cache`), read by the plan's common code
  (frame blending, `layer_src`), so the wand can read the source picture without a GPU readback.
- **The screen eyedropper** (F-MSK-007) and masks edited on the preview (shape-masks).
- **Links** (R.61) and the point tracker design (change `point-tracker`) — a seed can be linked to a track.

### What is missing

- A mask shape driven by pixels; the coverage memo keyed by the frame's content.

### What hinders

- A wand on an effect slot after other effects would need the GPU's intermediate picture on the CPU;
  the wand reads the SOURCE picture (the event's decoded frame) — stated, not hidden.

### Which invariants it touches

- **7** — computed in the plan's common code from the source frame (the frame-blending precedent), applied
  by the one mask pass.
- **4 / 11** — the coverage is a pure function of (frame content, seeds, parameters), memoized by them.
- **12** — at rest nothing recomputes; dragging the tolerance recomputes the shown frame only.
- UI rules 13, 17, 22.
- CAPABILITY: openspec/specs/video-fx/

## Estimate

| What | Answer |
|---|---|
| Scope | M — a mask shape, the fill, the edge, the UI clicks |
| Where | kt-fx (`mask.rs`), kt-model (`MaskShape::Wand`), kt-engine (source pixels into the coverage), kt-app |
| Project format | a new version by content (a wand mask) |
| Tests | fills on synthetic frames; per-frame recompute with a moving seed; determinism; idle |
| Depends on | F-MSK-001 (done), F-MSK-005 (for moving seeds) |
| Breaks compatibility | no |

## Verdict

Take it as an algorithmic mask shape — contiguous or colour-range, seeds that can follow a point track,
an anti-aliased edge from the colour distance — computed per frame from the source picture. A neural
object segmentation is not taken now, for the reason the owner accepted for the pro keyer (2026-11-01):
the same bytes on every machine, no ML runtime and weights in the program; it may come later as an assist.

## Sources

- The owner's request, `docs/ktprompts/kkt-26.11.01.txt` line 71 (registry G1).
- `src/libs/kt-fx/src/mask.rs`, capability `video-fx` (shape-masks).
- Editor rows from prior knowledge, [NOT VERIFIED].
