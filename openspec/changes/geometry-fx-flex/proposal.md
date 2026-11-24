# Proposal: geometry-fx-flex

> DESIGN, revision 2 (2026-11-24), base `229c694b`: implementation-ready after `preview-canvas-and-gizmos`. The owner answered Q1–Q2
> (A24). Code starts on the orchestrator's go. Tests: t3570–t3576, t4245.

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

- **Mirror gains a reflect mode**: a line through a centre at an angle; the kept side stays, the other shows its reflection, which may
  also be flipped along the line. Old slots keep the flip byte for byte; a Mirror from the picker reflects.
- **EventGrid gains** flips of every other column and row, a gap, a phase that slides the content, a row shift.
- **A new Tile** (AE's Motion Tile): a source rectangle repeated in tiles INSIDE a destination rectangle, mirrored edges, an
  animated phase, a row shift; the picture outside the rectangle stays: "из верхней части сделать «пилу», чтобы там текстура ездила".
- **All are maps of the one fused geometry pass** (F-CMP-001), affine inside a cell, so no seam is blurred; his chain is one pass.
- **The audit** of the other legacy effects stays a list (F-FX-019).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-reach`: the line and the rectangles on the preview.
- `video-fx`: Mirror's reflect mode, EventGrid's flips/gap/phase, the Tile kind; their place in the fused
  geometry pass and the owner's chain as the acceptance frame.

## Impact

- kt-plugin-api: `GeoStep::{Reflect, Cells}`, `HandleDecl::Rect`; kt-fx `geom.rs` ↔ kt-gpu `geom_fused.wgsl` (one commit).
- `src/plugins/fx-video-distort`: Mirror and EventGrid rows, the `tile` plugin, variants.
- kt-app: the rectangle handle, i18n ×5. Format: the next free version by content. Idle: nothing at rest.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 35, A24): "Q44/Q45 Mirror: «Старые отражают как раньше, остальное потом
(Recommended)»" — Q1 (a), old Mirrors keep flipping, a new one reflects; Q2 (a), the rest of the audit later. Read again on
2026-11-24 with his whole items (`kkt-26.11.01b.txt` lines 7–9): "остальное" is the audit of the other effects, not the grid and the
repeat he asked for; B8's "из верхней части … там" needed a destination rectangle, B7's "вертикальное отражение отзеркалить" a flip
of the reflected side (design D0.1, D0.2). No open question.

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
