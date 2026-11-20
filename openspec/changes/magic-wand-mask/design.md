# Design: magic-wand-mask

> DESIGN ONLY, revision 1 (2026-11-20), base `3556dd30`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options.

## Context

Read on `3556dd30`:

- `kt_fx::mask` (`src/libs/kt-fx/src/mask.rs`): ONE CPU function turns an ordered mask list into 8-bit
  coverage; kt-gpu uploads it and applies it in one integer pass (`mask.wgsl`), conformance EXACT; only
  IEEE-exact operations and the frozen math of `kt_model::mask`.
- `kt_model::mask` (`src/libs/kt-model/src/mask.rs`): `MaskShape::{Rect, Ellipse, Bezier}`, `Mask { id,
  name, shape, mode, invert, opacity, feather, falloff, … }`, rows by stable ids, curves in `Mask.auto`
  (material domain), links via `ParamOwner::Mask`.
- `ClipMaskPlan` (kt-engine) maps the event's masks onto the materialized chain: the picture right after
  the leading Geometry node, a slot at its position; `LayerSrc.masks` carries them evaluated at the
  frame's material tick.
- The pre-FX frame is CPU bytes in the frames cache; `layer_src` reads it for frame blending.

## Goals / Non-Goals

**Goals:** a contiguous or colour-range region from clicks; an anti-aliased edge; per-frame recompute;
moving seeds through the existing curve/link machinery; exact bytes.

**Non-Goals:** neural segmentation (Q1); temporal smoothing of the matte (it would make a frame depend on
its neighbours — a later option keyed honestly); reading the picture after earlier effects (Q2).

## D1. The shape

```text
MaskShape::Wand { seeds: Vec<Seed>, tol: f64 /* 0..100 */, contiguous: bool, space: WandSpace,
                  soft: f64 /* 0..100: width of the anti-alias ramp in tolerance units */,
                  grow: f64 /* ref px, ± */, fill_holes: bool, analysis: u32 /* max width, default 960 */ }
Seed { id: u8, x: f64, y: f64 /* normalized to the owner frame */, sub: bool }
WandSpace = Rgb | Luma | Chroma
```

The seed colour is NOT stored: it is read from the frame at the seed every frame (that is what makes it
follow lighting changes). Rows: `tol`, `soft`, `grow`, `s<id>.x`, `s<id>.y` — smooth rows that take keys,
formulas and links (a seed linked to a tracker point follows it).

## D2. The fill

`wand_coverage(src: &[u8], w, h, shape) -> Coverage`:
1. downscale the source to `analysis` width by an integer box mean (the scopes rule) — the cost bound;
2. per pixel the colour distance to each seed's colour (a 3×3 mean at the seed): Rgb — max channel
   distance, Luma — |ΔY|, Chroma — max(|ΔCb|, |ΔCr|) (integer Rec.709);
3. contiguous: a scanline flood fill from each `add` seed over pixels with distance ≤ tol + soft (the
   ramp band is included so the edge is reachable); colour range: every pixel; subtract seeds the same and
   are removed;
4. alpha = 255 inside `tol`, a linear ramp to 0 across `soft`, 0 beyond — the anti-aliased edge;
5. fill holes (enclosed zeros of the region → 255, a border flood of the complement), grow/shrink by a
   distance transform (two-pass integer chamfer) in analysis pixels scaled from ref px;
6. upscale to the coverage raster bilinearly with integer weights; then the mask list's usual feather,
   opacity, invert and mode apply.

Integer and IEEE-exact operations only: the same bytes on every platform (t3537).

## D3. Where its pixels come from (Q2 a)

The mask plan gives the rasterizer the event's decoded SOURCE frame (the bytes the cache holds for the
layer, at the decode size) for a picture or slot wand; a track wand reads the track's composite input —
not available on the CPU in the GPU executor, so a track wand is refused in the first stage with its reason
(a validate error naming the track). The wand's seeds are in the owner frame's normalized coordinates and
are mapped into the source by the inverse of the leading Geometry node (crop, pos, zoom) for a picture
mask, so the click lands where the person clicked on the canvas.

## D4. Cost and memo

At 960 px analysis width: ≈ 0.5 M pixels × a few integer ops — 1–3 ms per frame on one core. The coverage
memo (`MaskMemo`, bounded, in the ledger) is keyed by (the frame's content serial, the resolved wand values
at the tick, the coverage size); a static seed on a still or paused frame is computed once. The layer's
`fx_hash` already includes masks; a wand makes the layer's key depend on the frame content — which the
plan key already does through the content serial.

## D5. The interface

Preview right click → Mask → "Magic wand": the click becomes the first seed; Shift-click adds a seed,
Alt-click a subtract seed; dragging from a seed sets the tolerance (distance from the seed in screen px →
tol), shown live on the paused frame (each drag step recomputes the shown frame only); the mask's form
(existing mask fields) shows tol, soft, grow, contiguous, space, fill holes; a seed's context menu offers
"Link to tracker point…" (change `point-tracker`).

## D6. Format, OTIO, idle

`.kt`: the next free version by content (`has_wand_masks()`). OTIO: masks are carried only in metadata
already (row "shape-masks"); the wand is added to that row's meaning — a planned row says so. Idle: a paused
frame computes once; hover redraws nothing; the live `idle` scene adds a wand mask on a paused frame.

## D7. Class walk — "a mask whose coverage depends on pixels"

| Place that caches a mask's coverage | Keyed by shape only? | Fix |
|---|---|---|
| `MaskMemo` / `MasksPlan` (compiled rows) | yes today | + content serial for a wand |
| present memo / readback memo | plan key with content serials | fine |
| disk render cache (`durable_hash`) | leaves note the file | fine (the file determines the pixels) |
| export | composes per frame | fine |
| the bin poster / reel strip | offline plans | fine |

## Rejected

- **A GPU flood fill** — contiguity is serial; fragment passes would take hundreds of ping-pongs; the
  coverage is CPU by the mask rule anyway.
- **Storing the seed colour** — the region would stop following lighting.
- **Temporal propagation** — a frame would depend on others' results; the seed-follows-track form keeps
  frames independent.
