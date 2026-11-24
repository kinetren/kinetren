# Proposal: geometry-fx-flex

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3570–t3584.

## Why

The owner, `kkt-26.11.01b` B6–B8 (lines 7–9; registry `kkt-26.11.01b-index.md`): "тот же mirror - очень
скудный, в том же старейшем sony vegas можно назначить угол отражения, а тут нет, через коледоскоп
приходится мучаться; еще про тот же eventgrid тоже скудный как и другие эффекты - у них нет гибкости … я вот
хочу расклонировать в grid, отзеркалить по вертикали, и вертикальное отражение отзеркалить по горизонтали - а
я не могу … мы не дотягиваем ни то что до AE, а даже до Vegas … нету банального эффекта repeat-текстуры,
чтобы можно было по ней ездить туда-сюда, например после отзеркаливания я хочу из верхней части сделать
"пилу", чтобы там текстура ездила". Taken (user-feedback, "the second list"): one change against Vegas and
AE — Mirror with an angle and a centre; a grid with per-row and per-column flips; a repeat texture with
mirrored edges and a phase to animate (AE Motion Tile); his example is the acceptance scenario; the other
legacy effects get a parameter audit. Note F-FX-019.

## What Changes

- **Mirror gains a reflect mode**: a line through a centre (x, y) at an angle; the side the line's normal
  points to is kept and the other side shows its reflection; a "both" option reflects the other way. The old
  flip (horizontal/vertical flags) stays the mode of existing slots, byte for byte; a Mirror added from the
  picker starts in reflect mode (Q1).
- **EventGrid gains flips and motion**: flip every other column, flip every other row (a seamless pattern),
  a gap between cells, a phase x/y that slides the content inside the cells, a row phase shift (bricks).
- **A new Tile** (AE's Motion Tile): a source rectangle of the picture (default the whole frame), the tile
  size, mirrored edges, phase x/y and an alternate-row shift; it fills the canvas. Repeating the top half
  with mirrored edges and an animated phase is the owner's "saw".
- **All are inverse maps of the one fused geometry pass** (concatenation, F-CMP-001): the owner's chain is
  one pass with supersampling by the local scale.
- **An audit table** of the other legacy effects against Vegas and AE (in the design), each gap a row for
  the catalogue waves (F-FX-007), not built here.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: Mirror's reflect mode, EventGrid's flips/gap/phase, the Tile kind; their place in the fused
  geometry pass and the owner's chain as the acceptance frame.

## Impact

- kt-plugin-api: `GeoStep::Reflect { cx, cy, angle, both }`, `GeoStep::Grid` gains `flip_x, flip_y, gap,
  phase, row_shift`, `GeoStep::Tile { src: Rect, tile: (w, h), mirror, phase, row_shift }`.
- kt-fx `geom.rs` ↔ kt-gpu `geom_fused.wgsl` (a pair, one commit): the new maps, their Jacobians for
  supersampling, the uniform budget.
- `src/plugins/fx-video-distort`: Mirror rows (`mode`, `cx`, `cy`, `angle`, `both`), EventGrid rows, the Tile
  plugin; the centre on the preview (F-UI-009's crosshair) for Mirror and Tile; picker variants.
- Format: new rows omitted at defaults; a file with any is written with the next free version by content.
- Idle: geometry is a function of the frame; nothing runs at rest.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20` A24, his Q44–Q45): "Старые отражают как раньше, остальное потом" — Q1 (a), old
Mirrors keep flipping, a new one reflects; Q2 (a), the rest of the audit as ranked rows, built later.

The questions as asked:

**Q1. A Mirror added from now on reflects; old Mirrors keep flipping?** Your words: "в sony vegas можно
назначить угол отражения". Today's Mirror flips the whole frame (v1's port); Vegas/AE reflect across a line.
- (a) **Old slots keep the flip; a new Mirror starts in reflect mode (angle 90°, centre in the middle); the
  form has a mode switch** (*recommended*: rule 20, old projects unchanged).
- (b) One mode only: reflect; old flips are converted on open (a flip equals a reflect only for whole-frame
  symmetric pictures — the look of old projects changes).

**Q2. The rest of the audit** (Kaleidoscope, Twirl, Ripple, Lens, Pixelate, Halftone, Vignette … against
Vegas and AE): built in this change, or listed as rows for the next waves?
- (a) **Listed in the design's table and the catalogue (F-FX-007) as ranked rows; built one by one later**
  (*recommended*: this change stays the geometry trio your example needs).
- (b) Build every gap now.
