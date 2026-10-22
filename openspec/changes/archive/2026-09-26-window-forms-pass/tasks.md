# Tasks

## 1. Form building blocks and the contract guard

- [x] 1.1 `widgets.rs`: a common row for a quantity WITHOUT a passport — `plain_row(label, value, unit, range, step)` (the unit and the limits as arguments from one place) and the pair row `pair_row` for two values on one row; both return "touched" with the same type as `param_row_bound`. Check: `cargo test -p kt-app` is green, the new rows are covered by t779 below.
- [x] 1.2 The contract guard, the part "not a single own number": the kt-app source scanner fails on `egui::DragValue::new` outside `widgets.rs`, with the address of the place. Test t779 (`src/apps/kt-app/src/reach_tests.rs`); RED before the windows were edited — the program currently has 15 such places.
- [x] 1.3 The contract guard, the part "buttons are reachable": the harness opens EVERY window of the `WINDOWS` registry in a low window, finds its action button, presses it, and requires the consequence (the window closed / the edit arrived). Test t779 (continuation); red on windows without scrolling.

## 2. The contract across all windows

- [x] 2.1 Body scrolling and the buttons OUTSIDE it — in all windows of the registry where they do not yet exist (`fx_editor`, `morpher_editor`, `transition_editor`, `camera_editor`, `settings_window`, `render_dialog` ×2, `envclip`, `pubparam` ×3, the modals of `app.rs`). Check: t779 is green, the previous tests of these windows (t716b, t743, t744, t752b, t754) do not turn red.
- [x] 2.2 Replacement of their own `DragValue`s by the common rows: morpher (4), FX window (4), timeline (3), camera (3), automation (1). Limits and units — from the passport where it exists; where it does not — through `plain_row` with arguments from one place next to the definition of the quantity. Check: t779 is green; the values and limits are the same as before (t265, t281, t698 do not turn red).

## 3. Six dense forms on the model of "Transform"

- [x] 3.1 The effect chain window (`fx_editor.rs`): groups for the parameters of a kind, pairs on one row where the values are paired, a slider for quantities with both limits. Test t780 (`src/apps/kt-app/src/layout_tests.rs`), the FX part.
- [x] 3.2 The morpher (`morpher_editor.rs`): groups "source / targets / patch", pairs, units from the passport. Test t780 (continuation); t35–t36 (the morpher layer) do not turn red.
- [x] 3.3 The transition (`transition_editor.rs`) and the camera (`camera_editor.rs`): groups and pairs, a slider for bounded quantities, the schema — where it already exists. Test t780 (continuation).
- [x] 3.4 Project settings (`settings_window.rs`) and render (`render_dialog.rs`): the groups "frame / sound / conform rule" and "file / video / sound", units for each number. Test t780 (continuation); t722b (a change of rule rebuilds the preview) is green.
- [x] 3.5 New labels — into all five catalogs; the font subsets are rebuilt (`python3 scripts/subset-fonts.py`). Tests `i18n_tests.rs`, `font_tests.rs`.

## 4. A small frame gives its handles outward

- [x] 4.1 `kt-fx/src/geom.rs`: a pure function "the outward normal of a handle" (the direction of the offset for each of the eight handles, taking rotation into account). Test t782 (`src/libs/kt-fx/tests/placement.rs`): the directions are orthogonal to the edges and rotate together with the frame.
- [x] 4.2 `geomctl.rs`: when the side of the frame ON SCREEN is smaller than `3 × GRAB`, the scale handles are drawn and caught with an offset of `GRAB` outward; the body of the frame remains under the carry-over; the anchor and the rotation stay in place. Test t781 (`src/apps/kt-app/src/layout_tests.rs`): on a small frame the centre gives the carry-over, the handles are caught from outside; on an ordinary one — everything as before (t763 is green).

## 5. Live scenario and finish

- [x] 5.1 `src/scripts/live/forms.py`: open the FX window, the morpher and the project settings in a LOW window (a forced size), make sure the buttons are visible and pressable, collapse and expand a group, adjust a number with the slider and see the consequence in the file after Ctrl+S. Run — `./scripts/live-check.sh forms`.
- [x] 5.2 The capability specs are updated by the delta, `openspec validate --all --strict`; in `openspec/specs/ui-reach/design.md` and `ui-timeline/design.md` the "why" is recorded (the form contract in one place, the guard checks the consequence, the ban on `DragValue`, moving the handles outward by the screen size).
- [x] 5.3 `cargo test --workspace` (kt-app per crate under the SIGBUS flake), `cargo clippy --all-targets`, `./scripts/live-check.sh` (all scenarios), `./src/scripts/finish.sh` — the version in the report; a commit of the stage; open the owner's project with the built kt-app and say in the report what is visible (including the small frame on his `paintitblack.otio`).
