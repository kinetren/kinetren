# Design: geometry-fx-flex

> DESIGN ONLY, revision 1 (2026-11-20), base `43aa8a08`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options.

## Context

Read on `43aa8a08`:

- `src/plugins/fx-video-distort/src/lib.rs`: `HostStep::mirror()` — rows `horizontal`, `vertical` →
  `GeoStep::Mirror { h, v }`, alone without a scale change the exact permutation kernel (`Lone::Mirror`,
  t297); `HostStep::event_grid()` — rows `cols`, `rows`, `center` → `GeoStep::Grid`; warps (`Warp`) for
  lens/twirl/ripple/kaleidoscope/cylinder with a WGSL mirror each; Corner Pin as `GeoStep::Proj`.
- `src/libs/kt-fx/src/geom.rs`: consecutive Pan/CamMotion/Mirror/EventGrid compose into ONE pass of inverse
  maps; the tile modulo of EventGrid works in "d" coordinates; supersampling taps by the Jacobian at
  minification; the uniform's capacity bounds a series (worst case six EventGrids); a series of length 1 of
  Pan/CamMotion/Mirror keeps the old passes (bytes as before R.44). Its WGSL twin is
  `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` — they change in one commit (t181 geo cases, `FrameTol::GEOM`).
- The effect centre on the preview (F-UI-009): every `_x`/`_y` pair is a crosshair on the preview.

## Goals / Non-Goals

**Goals:** the three operations as exact inverse maps in the fused pass; the owner's chain renders in one
pass; old slots keep their bytes.

**Non-Goals:** building the audit's other gaps (Q2); a planar "mirror planes" multi-reflection (Resolve's
Mirrors) — expressible by chaining reflects.

## D1. Reflect (Mirror, mode reflect)

Normalized centred space (the fused pass's own), the line through `c = (cx, cy)` with unit normal
`n = (cos θ, sin θ)` (θ = angle; 90° = a vertical line, the left kept). Inverse map for an output point p:

```text
s = dot(p − c, n)
src = p                     if s ≥ 0  (the kept side)
src = p − 2·s·n             otherwise (the reflection)
both: the sign test reversed
```

Piecewise affine: the Jacobian is ±rotation (determinant ±1), supersampling unaffected; the seam is exact
(both branches agree on s = 0). With θ ∈ {0°, 90°} and c at the centre the map is a half-frame permutation;
it still goes through the fused pass (not the flip's lone kernel), integer-exact on the grid points.

## D2. Grid flips, gap, phase

Per cell (i, j) of the existing tile modulo: local coordinate u ∈ [0, 1)²; `flip_x` and odd i → `u.x := 1 −
u.x`; `flip_y` and odd j → `u.y := 1 − u.y`; `gap` (px) shrinks the picture inside each cell, the gap
transparent; `phase` (fractions of a cell) is added to u before the modulo (the content slides through the
cells, the grid stays); `row_shift` adds `j · row_shift` to u.x (bricks). Defaults reproduce today's map
exactly (t3571).

## D3. Tile (Motion Tile)

Rows: `src_x, src_y, src_w, src_h` (normalized source rect, default the whole frame), `tile_w, tile_h`
(fractions of the canvas, default 1), `mirror` (flag), `phase_x, phase_y` (tiles), `row_shift`. Inverse map:
the canvas point → tile coordinates `t = (p − origin)/tile + phase`; integer cell k = floor(t), local f = t − k;
with `mirror`, odd k.x → f.x := 1 − f.x (likewise y) — the seamless "saw"; `row_shift` offsets by k.y; then
`src = src_xy + f · src_wh`. The modulo map's Jacobian is the source/tile scale, so supersampling follows
`src_wh / tile` (the minification case of a small tile).

## D4. The owner's chain — the acceptance frame

EventGrid (3×3, flip_x, flip_y) → Mirror (reflect, 90°, centre) → Mirror (reflect, 0°, centre) → Tile (src =
the top half, mirror, phase_x animated 0 → 2 over the event): one fused pass (four steps within the uniform
budget), compared with the four steps run as separate passes (within `FrameTol::GEOM`) and with a CPU closed
form of the composed map (t3574). Its frames are stored as references.

## D5. Rows, UI, format

- Mirror: `mode` (`flip` | `reflect`; absent = `flip`), `cx`, `cy`, `angle`, `both`; the picker's variants
  "Reflect left/right", "Reflect top/bottom", "Reflect at an angle" set `reflect`; the centre is the preview
  crosshair, the angle a handle on it (the geometry frame's rotate handle, R.64).
- EventGrid: `flip_x`, `flip_y`, `gap`, `phase_x`, `phase_y`, `row_shift` (all omitted at 0/false).
- Tile: a new plugin `tile` in the distort family; its centre `src` rect drawn on the preview as a frame.
- Format: rows omitted at defaults; a file with any new row (or a `tile` slot) is written with the next free
  version by content (an older build would drop the rows).

## D6. The audit of legacy effects (Q2 a — listed, not built)

| Effect (ours) | Our rows (read in the plugin crates) | What Vegas / AE add [NOT VERIFIED live] | Row for the waves |
|---|---|---|---|
| Kaleidoscope | segments, angle | AE CC Kaleida: size, rotation, centre, mirroring type; Vegas: centre, offset | centre on the preview, mirroring type |
| Twirl | angle, radius | AE Twirl: centre | centre on the preview |
| Ripple | amount, freq, phase | AE Ripple: centre, wave type, wave speed (animates without keys), conversion | centre, wave speed, wave type |
| Lens distort | k1, k2 | AE Optics Compensation: field of view, reverse distortion, centre | FOV units, reverse, centre |
| Pixelate | size | Vegas Pixelate: separate horizontal/vertical size | x/y sizes |
| Halftone | size, angle (per screen) | AE Color Halftone: per-channel angles, dot shape | dot shape |
| Vignette | strength, radius, softness | Vegas/AE: roundness, centre, colour | roundness, centre, colour |
| EventGrid | cols, rows, center | (this change) | — |
| Mirror | horizontal, vertical | (this change) | — |
Rows marked [NOT VERIFIED] in the note; the catalogue (F-FX-007) gets the ranked list when the owner says (Q2).

## D7. Class walk — "a geometry effect too rigid to chain"

| Step | Fuses? | Here |
|---|---|---|
| Pan / CamMotion / Geometry node | yes | unchanged |
| Mirror flip | yes, lone exact kernel | unchanged for old slots |
| Reflect | new, fused | D1 |
| EventGrid | fused | D2 |
| Tile | new, fused | D3 |
| Kaleidoscope (warp) | fused as a warp | its own audit row |
| Corner pin | fused (Proj) | unchanged |

## D8. Idle

A geometry pass per composite; no state, no history; nothing at rest.

## Rejected

- **Converting old flips to reflects** (Q1 b) — rule 20; the bytes of old projects.
- **Separate passes for the new steps** — intermediate rasterization and 8-bit rounding (F-CMP-001).
