---
id: F-FX-019
title: Mirror with an angle, a grid with flips, a sliding repeat texture
status: candidate
priority: B
cost: M
category: fx/distort
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugin-api, kt-fx, kt-gpu]
invariants: [7, 11]
ui_rules: [9, 20]
round: null
point: null
tests: []
depends_on: [F-CMP-001]
checked: 2026-11-20
---

# F-FX-019 — Mirror with an angle, a grid with flips, a sliding repeat texture

## Essence

Three geometry tools people combine: a mirror that reflects the picture across a line at any angle through
a chosen centre (one half kept, the other its reflection); a grid that repeats the picture in cells with
every other row or column flipped (a seamless pattern); a repeat texture (tile) that repeats a chosen part of
the picture and slides it by an animatable phase. The owner's example chains them: a grid, a vertical mirror,
that reflection mirrored horizontally, then the top half repeated as a sliding "saw".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Mirror; (no tile; "Picture in Picture" for cells) | Mirror: orientation and a mirror plane offset/angle — the reflection across a line [NOT VERIFIED live] |
| Adobe Premiere | 26 | Mirror; (Motion Tile via AE only) | Mirror: Reflection Center (point) and Reflection Angle (degrees) [NOT VERIFIED live] |
| DaVinci Resolve | 21 | Mirrors (ResolveFX), Fusion Duplicate/Tile | Mirrors: several mirror planes with angle and position; tiling in Fusion [NOT VERIFIED live] |
| Final Cut Pro | 12 | Mirror, Kaleidotile | Kaleidotile: segment angle, offset, rotation, tile [NOT VERIFIED live] |
| After Effects | 26 | Mirror; Motion Tile; CC Kaleida | Motion Tile: tile centre, tile width/height, output width/height, **Mirror Edges**, **Phase** with **Horizontal Phase Shift** — the phase animates the slide [NOT VERIFIED live] |

## The trick behind it

**1. "Mirror" means two different operations, and our v1 port has the wrong one for the request.** Our
Mirror FLIPS the whole frame (a permutation). Vegas/AE/Premiere's Mirror REFLECTS across a line: the side of
the line the person keeps stays, the other side becomes its image. The angle and the centre only make sense
for the second; with a flip they are meaningless — that is why the owner had to "мучиться через калейдоскоп".

**2. A seamless repeat comes from mirrored edges, not from the tile itself.** AE's Motion Tile has "Mirror
Edges": every other tile is flipped, so the edge of one tile meets its own mirror image and no seam shows;
with it, any piece of a picture becomes a seamless texture. The "saw" in the owner's example is exactly a
mirrored-edge tile of the top half.

**3. The phase is what makes a tile move without an animated position.** Phase slides the content inside
the fixed tile grid (and Horizontal Phase Shift offsets alternate rows), so a texture "rides" back and forth
with one curve, and the grid itself never moves.

## Pros

- Each is an inverse coordinate map: they fuse with Pan/Mirror/EventGrid into the one geometry pass (F-CMP-001),
  no intermediate rasterization, exact chains like the owner's at the cost of one pass.

## Cons

- Folding and tiling are non-linear maps (abs/mod): supersampling at minification must follow the local
  scale, as for the kaleidoscope.

## What users say

The owner, `docs/ktprompts/kkt-26.11.01b.txt` lines 7–9: "текущие эффекты, вот тот же mirror - очень скудный,
в том же старейшем sony vegas можно назначить угол отражения, а тут нет, через коледоскоп приходится
мучаться… я вот хочу расклонировать в grid, отзеркалить по вертикали, и вертикальное отражение отзеркалить по
горизонтали… нету банального эффекта repeat-текстуры".

NO QUOTES: no web access in this spec session (2026-11-20) for outside quotes; the editor rows are [NOT
VERIFIED].

## How it fits Kinotrain

### What already exists

- `src/plugins/fx-video-distort/src/lib.rs`: `HostStep::mirror()` (flags horizontal/vertical → `GeoStep::Mirror`,
  the exact lone kernel t297) and `HostStep::event_grid()` (cols, rows, center → `GeoStep::Grid`); the warp
  family (kaleidoscope's triangle wave).
- The fused geometry pass: `src/libs/kt-fx/src/geom.rs` ↔ `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` (a pair
  that changes together), concatenation of series, supersampling by the Jacobian.

### What is missing

- A reflect step (line through a centre at an angle), per-row/column flips and a phase in the grid, a tile
  step with a source rectangle, mirrored edges and phases.

### What hinders

- The fused pass's uniform capacity (`geom.rs`: "worst case — 6 EventGrid"); new steps must fit the budget or
  split the series.

### Which invariants it touches

- **7** — one fused pass; the CPU geometry and the WGSL mirror change as a pair.
- **11** — the new maps use the frozen math (`kt_plugin_api::math`).
- CAPABILITY: openspec/specs/video-fx/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-plugin-api (`GeoStep`), kt-fx/geom.rs + geom_fused.wgsl, the distort family |
| Project format | new rows, omitted at defaults; version by content |
| Tests | the owner's chain as the acceptance frame; old Mirror/Grid bytes unchanged; fused vs separate passes |
| Depends on | F-CMP-001 (done) |
| Breaks compatibility | no |

## Verdict

Take it: Mirror gains a reflect mode with angle and centre (the flip stays for old slots), EventGrid gains
per-row/column flips, a phase and a gap, and a new Tile (Motion Tile with a source rectangle, mirrored edges
and phases); all three are steps of the fused geometry pass. The other legacy effects get a parameter audit
against Vegas and AE in the change's design.

## Sources

- `docs/ktprompts/kkt-26.11.01b.txt` lines 7–9; `docs/user-feedback.md` "the second list" (taken 2026-11-01).
- `src/plugins/fx-video-distort/src/lib.rs`, `src/libs/kt-fx/src/geom.rs`.
