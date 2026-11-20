# Design: fx-form-width

> DESIGN ONLY, revision 1 (2026-11-20), base `756f41b5`.

## Context

- `widgets::params_grid_bound` (`src/apps/kt-app/src/widgets.rs`): `egui::Grid::new(id).num_columns(4)` —
  label, slider, value, curve — one parameter per row, whatever the width.
- `widgets::group` (the same file): a `CollapsingHeader` whose state is the window's (R.66, t780).
- The window shell: every window resizable, the body fills it, sizes remembered per kind
  (windows-fill-their-area, t1125, t1127 for declared fixed sizes).
- The form contract t779: buttons outside the scroll, numbers only through the shared row.

## D1. The flow

`FormFlow { min_col: f32 }` computed once per window frame from the catalogue's longest label of the rows
shown (measured galleys, cached per language and kind) + 120 pt slider + the value field + the curve button.
`cols = clamp(floor(avail_w / (min_col + gap)), 1, MAX_COLS = 4)`. Rows of one group are distributed into
`cols` columns top-to-bottom (column k holds rows `[k·n/cols, (k+1)·n/cols)`); groups are cards placed in a
masonry by shortest column; a group with fewer rows than `cols` takes one column. Each column is its own
`egui::Grid` with the same 4 sub-columns, so labels align within a column. Value pairs (`_x`/`_y`, R.66)
stay on one row.

## D2. Where it applies

Every caller of `params_grid`/`params_grid_bound` and every form built of `group` cards: effect slots
(chain and node editor), morpher patches, transitions, camera, track window, generators and titles, project
settings, render window. The ISP pipeline panel's tiles keep their tile layout; a tile's expanded rows use the
flow inside the tile. Forms that are not row lists (the EQ graph, the deck face of VHS) are outside the rule
and declared so in the guard.

## D3. The guard (t3591–t3593)

A harness test enumerates every registered kind (effects, transitions, generators) and every `WINDOWS` window
with a scene that opens it; at widths 420, the window's default and 1600 pt (height fixed at 800) it reads
the laid-out rows' rectangles (`field_probe`) and checks:
1. unused width at 1600 pt (the window's inner width minus the union of the columns' rects) ≤ 25 %, unless all
   rows already fit in one screen height at the default width;
2. the body's content height at 1600 ≤ at the default ≤ at 420;
3. every row's slider ≥ 120 pt;
4. t779's buttons visible.
A kind or window that cannot pass is listed in `FORM_EXEMPT` with a reason ≥ 25 characters (t3594 holds the
list to existing names). The test fails on today's code for the camera ISP and the VHS tape forms (checked by
the executor before counting it written — the kt-app rule "a test MUST fail on the old code").

## D4. Class walk — "space given but not used"

| Place | Today | Here |
|---|---|---|
| effect forms (`params_grid_bound`) | one column | flow |
| groups (`group`) | stacked | cards in columns |
| project settings, render window | groups stacked | flow |
| the bin (width limit R.13, "do not squeeze") | the owner's limit | untouched (his limit stays) |
| the meters column | fixed by his decision | untouched |
| node editor (fx-graph) | its own canvas | outside the rule |
| Diagnostics graphs | share the height ("needle's eye") | untouched |

## D5. Idle

Layout runs only in a window's drawn frame; the label measurements are cached; no frame is asked at rest.

## Rejected

- **Fixed narrow windows** — the owner made windows fill their area; the fix is to use it.
- **A per-window hand layout** — the class returns with the next kind; one flow in the shared widget.

## Applied (2026-11-20, base `ea0aff6f`) — what the code does and where it differs

- **One field a row.** The shared grid drew two fields a row in ONE column: at 420 pt the camera ISP
  and the VHS tape rows reached x = 491 / 450, past their window. Now a grid row holds one field
  (an `_x`/`_y` pair stays one row), and the columns of the flow give the density back:
  `form_cols(avail, min_col)`, `min_col` = the longest label shown (measured in the current
  language) + the 72 pt number field (+ its pair). Rows fill column after column
  (`grid_part` per column, the same 4 sub-columns). The "120 pt slider" of D1 does not exist in the
  shared row (only the Transform form has sliders, and it keeps its own layout): check 3 of D3 reads
  "no row reaches past the narrow form's edge".
- **Group cards: `widgets::CardFlow`.** A card goes into the column that is shortest AS DRAWN (no
  estimate); `end` gives the parent the flow's whole width. A card's minimum is the window's own
  minimum width (the width its rows are made to fit): render 440, settings 420; the ISP's blocks
  measure their widest row. Applied to the ISP pipeline blocks, project settings and render.
- **Not moved:** the camera window's groups stand between free rows (the ortho switch, the length
  line) and stay stacked; its parameter grid flows inside its group. The morpher's groups wrap the
  node graph. Both pass the guard.
- **Found by the guard on the way:** the track window's two ducking compressors stood in a row of
  `vertical`s — the first took all the width left and the second got a sliver. Each half now
  gets an equal share.
- **The window half of the guard (t3593b)** reads each window body's scroll content: a window is
  set to its default width and 600 pt tall, then widened to 1600 pt; its form must be no taller
  (unless it only fills its body — the track window's graph canvas) and, if it scrolled, must
  get shorter. The render window with its groups stacked fails it: 696 pt in a 524 pt body, no
  shorter when wide.
- `t281` aimed at the "rate" field between the "rate" and "mix" labels on one row; it now aims
  at the field's own published rectangle (the assertion on the model is unchanged).
