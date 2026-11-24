# Design: geometry-fx-flex

> DESIGN ONLY, revision 2 (2026-11-24), base `229c694b`. Q1 a, Q2 a decided (A24). Implementation-ready after
> `preview-canvas-and-gizmos` (its `UiDecl.handles`).

## D0. What revision 1 said and the code (or his words) contradict

1. **Tile "fills the canvas with a source rectangle".** His B8: "после отзеркаливания я хочу **из верхней части** сделать «пилу»,
   чтобы **там** текстура ездила": the top part becomes the saw and moves, the rest stays. A tile that repeats the top half
   over the whole canvas destroys the mirrored bottom. Tile gets a DESTINATION rectangle: outside it the picture is as it was (D3).
2. **Two Mirrors in a row for B7.** "отзеркалить по вертикали, и **вертикальное отражение** отзеркалить по горизонтали": the object of the
   second mirror is the reflection. Two whole-frame reflects mirror the upper half too. Both readings are his words'; a
   reflect gets "also flip the reflected side along the line" (D1), so either is one slot.
3. **"Integer-exact, the lone kernel", `GeoStep::Grid` with more fields, a Tile step**: the Fold's arithmetic, `grid_tile` (t943, t944)
   and the exact `Lone::Mirror` (t297) are byte contracts of old slots. New rows at their defaults emit TODAY's steps; anything
   else emits one new step `Cells` (grid and tile are one map) or `Reflect`.
4. **Supersampling "by the Jacobian".** A program with a Warp or a Proj is per pixel by finite differences (`SsSpec::PerPixel`): at every
   tile edge the difference is the size of the tile, and the edge would be blurred by 16 taps. Both new primitives are affine
   inside a cell, so they stay in the `Const` class as the Fold does; only a Reflect off the axes over an anisotropic scale asks
   the probes (D4).
5. **A mirrored edge is not seamless by itself**: the final bilinear reads half a texel outside the source rectangle. `Cells` clamps
   to the rectangle's texel centres (D2).
6. **`cx`, `cy`, "the angle handle of R.64".** The preview's crosshair is the pair `center_x`/`center_y` in −1..1 (`centerctl::PAIR`);
   handles are `UiDecl.handles` (`Centre`, `Radius`, `Angle`, `Quad`) since `preview-canvas-and-gizmos`. A rectangle handle is
   added there (D5).
7. Row names and stores are frozen rows of plugin manifests now (`RowDecl`, omit-at-default), and a picker's start values are
   `UiDecl.template`.

## Context

`src/plugins/fx-video-distort/src/lib.rs` (`HostStep` :378, `mirror` :392, `event_grid` :407, `steps` :433, `lone` :447),
`kt-plugin-api` `GeoStep` (:1221), `kt-fx/src/geom.rs` (`GeomPrim` :480, `Fold` :494, `prim_slots` :794, the builder :1060–1240, the Const/PerPixel
choice :1244, `grid_tile` :1300, `map_pos` :1372, `MAX_PRIMS` 16, `MAX_PRIM_SLOTS` 46), `kt-gpu/src/engine.rs` (packing :160,
`geom_specialized` :1294 refuses a Fold), `shaders/geom_fused.wgsl` (kinds 0–5 in four bits; 6 and 7 are free), t181, t297, t943, t944.

## D1. Reflect (Mirror)

Rows after `horizontal`, `vertical` (bytes of old slots untouched): `reflect` (flag, default off), `center_x`, `center_y` (−1..1,
default 0), `angle` (deg, −360..360, default 90), `swap` (flag: keep the other side), `flip_along` (flag). With `reflect`
off: today's steps and `lone`. With it on: `GeoStep::Reflect { c, n: (cos θ, sin θ), swap, along }`, `lone` = None.

In the stage's pixel space (square pixels, so an angle is the angle on the screen), centre-space, `s = dot(p − c, n)`, negated
by `swap`:
```text
src = p                                  s ≥ 0
src = p − 2·dot(p − c, n)·n              s < 0
src = 2·c − p                            s < 0 and along      (mirrored, and flipped along the line)
```
θ = 90°: n = (0, 1) on the screen, the line is HORIZONTAL and the upper half is kept ("top → bottom"); θ = 0°: a vertical line, the
left kept. (Revision 1 had the angle of the normal and of the line mixed.) The form names them: "Line: horizontal / vertical / 33°".
Points mapped outside the stage clamp to its edge as Pan does. Trigonometry by `FrozenMath`, once, on the host; primitive kind 6, two
slots. A seam has no gap: both branches agree on s = 0.

Picker: `template: [("reflect", 1)]`; variants: today's three flips (with `reflect` 0), "Reflect top to bottom", "Reflect left to right",
"Reflect, turned" (`flip_along`), "Reflect at 45°".

## D2. Cells: the grid with flips, gap and sliding content

`GeoStep::Cells { dst, cell: (w, h), src, flip: (x, y), gap: (x, y), phase: (x, y), shift, outside: Black | Keep }`, primitive kind 7,
four slots (dst; cell + phase; src in input index space; flags, shift, gap). For an output point in `dst`:
```text
d = p − dst.origin;  k.y = floor(d.y / cell.h);  d.x += k.y·shift·cell.w;  k.x = floor(d.x / cell.w)
f = d / cell − k                                       // place in the cell, 0..1
gap: f outside [g/2, 1 − g/2] → transparent; else f = (f − g/2) / (1 − g)
f = fract'(f + phase)  with the cell's own k advanced when phase carries it over (the content slides, the grid stays)
flip.x and (k.x odd, counted with the carried phase) → f.x = 1 − f.x;  likewise y
src point = src.origin + f·src.size − 0.5, clamped to src's texel centres
```
Outside `dst`: black (EventGrid, as today) or the stage itself (Tile).

EventGrid rows after `center`, omitted at default: `flip_x`, `flip_y`, `gap` (fraction of the cell 0..0.9), `phase_x`, `phase_y` (cells,
−8..8, smooth), `row_shift` (cells, −1..1). All default → `GeoStep::Grid` (its bytes, `grid_tile` as before). Else `Cells` over
the same integer rectangle (`offx`, `offy`, `cw`, `ch`).

## D3. Tile (AE's Motion Tile), a new plugin `tile` of the same family

Rows: `dst_x, dst_y, dst_w, dst_h` (0..1, default the whole canvas), `src_x, src_y, src_w, src_h` (0..1, default the whole picture),
`tile_w`, `tile_h` (fractions of the canvas, 0.02..2, default 0.5), `mirror` (flag, default on), `phase_x`, `phase_y` (tiles, −64..64,
smooth), `row_shift`. `Cells` with `cell = tile`, `flip = mirror` on both axes, `outside: Keep`. Phase is a curve: there and back is 0 → 1 → 0.
Variants: "Whole picture 2×2", "Top half slides" (dst = src = the top half), "Bricks".

`alpha`: passes through, as the grid (R.53 item 417); a gap makes transparency (`Produces`, only when gap > 0).

## D4. The program

Supersampling Const: `Cells` multiplies the lens by `src.size / (cell·(1 − gap))`, `Reflect` by 1. A `Reflect` with `n` off the axes
where lx ≠ ly at that point → `PerPixel` with the probes of `probe_program_lens` (continuous there, so no seam blur). `grid_tile` stays
the Grid's alone. Slots: worst case of six steps 6 × 4 + 7 = 31 ≤ 46, prims 13 ≤ 16. `geom_specialized` refuses `Cells` (as Fold) and
writes `Reflect` out (it meets every frame of a mirrored event on llvmpipe). `lens` feeds the decode step of `clip_head_zoom` as Fold does.

## D5. Handles

Mirror (with `reflect`): `Centre(center_x, center_y)`, `Angle(angle, At::Centre)`; the line is drawn through them. Tile: the new
`HandleDecl::Rect { x, y, w, h }` (0..1 of the canvas: a frame with eight grips and a body) for `dst`, solid, and for `src`, dashed.
`preview-canvas-and-gizmos` draws declared handles for event, morpher and track; `Rect` is its fifth kind, added here.

## D6. The acceptance chain

EventGrid 3×3 (`flip_x`, `flip_y`) → Mirror (reflect, 90°, `flip_along`) → Tile (dst = src = the top half, tile 0.25 × 0.25, mirror, `phase_x` 0 → 1 → 0
over the event): one fused pass of three primitives and their affines, compared with the three run as separate passes (within
`FrameTol::GEOM`, the edges of cells excepted by a one-pixel mask) and with a CPU closed form; frames stored. The bottom half of every
frame equals the chain without the Tile, byte for byte.

## D7. Class walk: "a geometry effect too rigid to chain"

| Step | Here |
|---|---|
| Geometry node, CamMotion | unchanged |
| Mirror flip, EventGrid at defaults | unchanged bytes |
| Reflect, Cells, Tile | new |
| Pan 3D, Corner Pin, Cylinder | a homography places the whole picture; a Tile BEFORE them gives what AE's repeat does |
| Kaleidoscope, Twirl, Ripple, Lens | no centre row: the audit (D8), his "остальное потом" |
| a masked slot, a slot in an arm of the graph | a masked Tile breaks the series (a Snapshot) and works; `dst` is the free form of it |
| strength of a morpher layer, transitions | as any geometry slot |
| generators (stripes, checker) | their own repeat, not this |

## D8. The audit (Q2 a: listed, not built)

The table stands in note F-FX-019 (Kaleidoscope: centre, mirroring kind; Twirl: centre; Ripple: centre, wave speed and kind;
Lens: FOV units, reverse, centre; Pixelate: x/y sizes; Halftone: dot shape; Vignette: roundness, centre, colour), each a row of F-FX-007.

## D9. Format, OTIO, idle

New rows and the `tile` tag are written only when used. An older build would open the file, draw a flip instead of a reflection and drop the
rows on saving, so a file with any of them is the next free version by content (**v+**). OTIO: effects travel in `metadata["kt"]` as all do; no row. Idle: geometry is a function of the frame.

## Rejected

- **Reflect and Tile as warp plugins** (seven numbers, no host change) — per-pixel supersampling blurs every seam (D0.4), and a tile's twelve
  numbers do not fit.
- **Converting old flips** (Q1 b) — he chose (a).
- **Separate passes** — an intermediate 8-bit raster per step (F-CMP-001).
