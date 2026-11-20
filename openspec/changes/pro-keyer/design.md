# Design: pro-keyer

> DESIGN ONLY, revision 1 (2026-11-20), base `70169e22`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options. Builds on `chroma-key-upgrade` (its difference model and Viewer views).

## Context

Read on `70169e22`:

- Barrier plugins (`src/libs/kt-plugin-api/src/barrier.rs`, plugins spec "A barrier kind is a plugin with a
  GPU pass graph", "may render at its own size", word passes): one input frame; host nodes `Resample`,
  `Premul`, `Save`; the K× rule is the planner's.
- Frame blending: two source frames per layer decided by ONE function `clip_frame_keys` for the order and
  the read (kt-engine CLAUDE.md) — the precedent for a held frame.
- The ISP's device-like panel (`src/apps/kt-app/src/isp_panel.rs`): stages in a row, each viewable.
- `chroma-key-upgrade`: the colour-difference model, Status view, Analyze.

## Goals / Non-Goals

**Goals:** uneven screens, hair, shadows, spill and bounce handled by one effect; deterministic; GPU; tunable
by sight.

**Non-Goals:** neural matting (the owner's base is an algorithm); light wrap (Q2); temporal matte
stabilisation; 3D keying (Primatte's polyhedron) — the plate-relative difference model covers uneven screens
more simply.

## D1. Linear light

Code values → linear by the sRGB-ish EOTF (a 256-entry table, exact), processing in 16-bit fixed point (word
passes), back at the end. Keying in linear light makes the shadow ratio a physical light ratio and the despill
restore preserve emitted brightness.

## D2. The clean plate

- **Generated** (default): a provisional screen-ness s0 with the global screen colour (from Analyze or the
  key rows); weights w = s0⁴ (only clearly-screen pixels vote); a pyramid down of (Σ w·c, Σ w) by 2× box to
  ~8×8, then up: at each level, where the level's weight is low, the coarser level's colour fills in
  (push-pull). The result is a smooth screen colour everywhere, the subject erased.
- **Held frame**: `held(p)` returns the material tick of the chosen moment; the plan fetches that frame of
  the layer's own source (the layer's decode size), the plate = that frame lightly blurred (3×3) to drop
  noise. A held frame from a moving camera gives a wrong plate: the panel says so (the plate view shows it).

## D3. Mattes

```text
e(P)   = P_k − (w·P_a + (1−w)·P_b)        e(C) likewise for the plate colour C at the pixel
s      = clamp(e(P)/max(e(C), ε), 0, 1)   // screen-ness against the plate
core   = fill_holes(choke(clip(1 − s, core_black, core_white), core_choke))
edge   = clip(1 − s, edge_black, edge_white)              // soft
band   = dilate(core, edge_band) − erode(core, edge_band)
alpha  = core outside the band; max(core, edge) inside it
```

Holes are filled by a coarse closing (dilate then erode at the band's scale) restricted to the core;
dilate/erode separable min/max (integer). Defaults from Analyze.

## D4. Shadows

Where s is high (screen), `ratio = Y(P)/Y(C)`; `shadow = clamp((1 − ratio)·shadow_gain, 0, 1)` softened by a
box blur; the output's alpha = max(alpha, shadow·shadow_amount) with colour black (a darkening layer over what
is below). `shadow_amount = 0` drops shadows entirely.

## D5. Spill and bounce light

On the subject (alpha > 0): `excess = max(0, e(P) − bias·e_ref)` removed from the key channel; luminance
restored to the original's (the upgrade's restore); the removed light replaced by `replace_mode`: neutral grey
of the same luma, a chosen colour, or `subject hue` — the mean hue of the pixel's neighbourhood where excess
is near zero (a 9×9 weighted mean at quarter size), so a green-lit shoulder returns to the shirt's colour.

## D6. Outputs and Viewer

Rows `view` are not rows: the Viewer (fx-graph v1) on a Pro Keyer node offers Composite, Matte, Status, Core,
Edge, Plate, Shadow, Spill — preview only, never rendered (as in `chroma-key-upgrade` D4).

## D7. The held frame contract

`BarrierKernel::held(&self, p) -> Option<HeldFrame>`; the plan's `clip_frame_keys` (ONE function for
`layers_at`/`wanted_frames` and the read) adds `(media, held_pts, quality)`; `max_concurrent_layers` counts it
(the export chunk budget, like a blending pair); the layer's content key includes the held frame's serial; the
pass graph sees it as input `Held`. Without a held frame nothing changes (bytes of every other barrier
unchanged).

## D8. The panel

The ISP-style pipeline (the simulation rule's device-like panel, though this is not a simulation): Plate →
Screen → Core → Edge → Shadow → Spill → Out, each stage a tile with its few rows and a "view" eye that sets the
Viewer; "Analyze" at the head; "Use this frame as the plate".

## D9. Cost

At 1080p ≈ 12 passes; the heaviest are the choke/fill (radius-bounded, separable) and the plate pyramid (log
size); comparable to the camera ISP. Measured per pass by the pass timer; the export uses the same graph.

## D10. Class walk — "a key that assumes an even screen"

| Place | Assumes one screen colour? | Here |
|---|---|---|
| greenscreen (distance) | yes | unchanged (old slots) |
| greenscreen (difference, upgrade) | yes, with balance | the simple key |
| Pro Keyer | no — per-pixel plate | this change |
| magic-wand mask colour range | seed colours | its own tool |

## Rejected

- **A neural matte as the base** — the owner's decision (2026-11-01); an assist later.
- **Primatte's 3D polyhedron** — hard to tune by sight; the plate-relative model divides the unevenness out.
- **Light wrap through a hidden read of the layer below** — breaks the single-input contract (Q2).
