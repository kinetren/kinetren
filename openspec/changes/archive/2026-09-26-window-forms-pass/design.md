# Design

## Context

See proposal.md — Why. Below is only what exists in the code and determines the shape of the solution.

- **The model is already written and accepted by the owner** — `src/apps/kt-app/src/geomctl.rs`, `transform_form`: `CollapsingHeader` with `id_salt` (collapsing is egui memory, that is, the state of the WINDOW), `geom_row` (a pair of values on one row), `shown_spec` (a unit from the passport, a fraction is shown as a percentage), `frame_schema` (the schema next to the numbers), a reset with one action by the same `CurveBind` rule.
- **The row with a slider is already shared** — `widgets::param_row_bound_ex(.., slider: Option<f32>)` (R.64): the number and the slider edit one value and write by the `CurveBind` rule.
- **Body scrolling is already done in one window** — `track_window.rs` (R.65): `ScrollArea` with `max_height = 0.62 × content_rect`, the buttons BELOW it. This is the model of the contract.
- **The window registry exists** — `reach_tests.rs`, `WINDOWS`: 21 entries; the t757 guard checks the presence of an entry, but not the kind of the form.
- **Precedents of a ban by a scanner**: `dnd_drag_source` is forbidden in the whole application (t254), direct forms `set_project(model.document())` are forbidden (t554). The same trick works for `DragValue::new` outside `widgets.rs`.
- **Quantities WITHOUT a passport exist**: the morpher's `source_in_delta` in ms, the camera duration in seconds, the timeline fields. They have no `ParamSpec`, and pretending that they do means inventing limits.
- **The frame handles are computed in fractions of the canvas** (`kt-fx::geom::Handle::point`), while the grab zone is in SCREEN PIXELS (`geomctl::GRAB = 14.0`). So a "small frame" is a property of the screen scale, and it must be decided where `to_screen` is known.

## Goals / Non-Goals

**Goals:**

- One form contract for all windows of the registry, and a guard that holds it.
- Six dense forms, laid out on the model of "Transform".
- The handles of a small frame — outside the edges, the body — under the carry-over.

**Non-Goals:**

- A single "inspector" instead of windows: the owner banned new windows, but did not ask to remove the existing ones — the request was about the LOOK of the forms.
- A re-layout of the other fifteen windows (question modals, log, search, diagnostics): they fall under the guard, but their forms are one or two rows, there is nothing to group there.
- Themes, colors, fonts, the set of parameters, new automation widgets.

## Decisions

- **The form contract lives in `ui-reach`, not spread across six capabilities.** The FX window is described in `ui-timeline`, the morpher in `morpher`, the transition in `transitions`, the camera in `video-fx`, the render in `export`. Writing "the body scrolls" into each of them would mean five copies of one rule (exactly what the owner scolded for as copy-pastes). The rule is one and lies where the window registry is; the scenarios of the individual windows refer to it.
- **The guard asserts the consequence WHERE IT IS OBSERVABLE.** Action buttons are checked by the consequence: the window opens in a LOW window, the button is searched for and must be within the bounds of the screen. But "part of the form is unreachable" is not visible to kittest at all (see "Lessons of execution": an egui window is clamped by the screen, clipped nodes come with truncated rectangles), therefore scrolling is checked structurally — by the states of the `ScrollArea` in egui memory — and next to it is a live scenario that turns the wheel in a real window.
- **`DragValue::new` outside `widgets.rs` is forbidden by the scanner.** This is the only way not to get a seventh form with its own limits through two rounds. The exception is exactly one — `widgets.rs` itself, where the row is defined.
- **A quantity without a passport is given either a passport OR an explicit row with arguments.** Where the value is a property of the model with a range (the camera duration, the morpher shift), a `ParamSpec` is created next to the type definition; where it is a one-off interface field, `widgets::plain_row(label, unit, range, step)` is used, and the limits come as arguments from ONE place. Inventing a passport where the quantity is not a parameter means lying about its nature.
- **Small frame: the handles are moved OUTWARD by screen size.** The threshold is a side of the frame smaller than `3 × GRAB` (42 px): below it the eight scale handles are offset by `GRAB` outward from their edges along the normals, and the hit test takes them at the same place. The alternative "reduce the grab zone" was rejected: 14 px was set by rule 1 of the checklist and by the owner's complaint about hitting with the mouse; the alternative "hide the handles on a small frame" was rejected — a gesture that is not visible does not exist.
- **The offset is computed by kt-app, not kt-fx.** `Handle::point` remains the geometry of the CANVAS (it is also called by the render), while moving outward is a screen-level correction: it depends on the scale of the preview, which kt-fx does not know. Only the pure function "the outward normal of a handle" (t782) moves into kt-fx, so that the direction of the offset is computed in one place together with the frame itself.
- **Grouping of groups is egui memory keyed by `id_salt`**, as in "Transform": this is the state of the window, not the project; it does not go into the file and does not go through undo.

## Risks / Trade-offs

- [The `DragValue` scanner will break the build on someone else's code that comes later] → that is the goal; the ban has one explicit address-exception and a clear replacement (the common row).
- [Re-layout of six windows — a lot of code without new behaviour] → each window is closed by its own assertion in t780 (groups, pairs, units, sliders) and by a live scenario; the risk of "rearranged and broke it" is caught by the previous tests of these windows (t716b, t743, t744 and kin).
- [Moving the handles outward gets in the way on a frame pressed against the edge of the picture] → the handles are drawn beyond the edge of the frame, but inside the preview area; for a frame in a corner they go to the edge of the picture, and this is visible — the behaviour is the same as Resolve's with a small PiP.
- [Opening 21 windows in one test is slow] → the windows are opened one by one, each frame is run twice; if the test turns out slower than a second, it is split by window.

## Lessons of execution (R.66)

- **A guard with no scene holds nothing.** The first version of t779 checked the buttons and was GREEN on windows with no scrolling at all: the windows it opened fit into a low screen. A dense scene (a chain of five effects) was needed — and a check of WHAT EXACTLY does not fit.
- **An egui window is clamped by the screen, and a "node beyond the edge" never appears**: clipped nodes come into the accessibility tree with TRUNCATED rectangles, and hidden rows do not disappear from the tree. The consequence "part of the form is unreachable" is not observable by kittest instruments, therefore scrolling is checked structurally — by the number of `ScrollArea` states in egui memory — and "is it not just empty space around it" is held by the live scenario `forms.py`, which in a real low window collapses a group and presses a button.
- **An exception must be MEASURABLE.** For a window that closes by the title's cross, and for a window that is short by construction, the reason is written in words AND checked by a number (the title is visible on screen; there are no more than 25 of its own rows). Otherwise the list of exceptions becomes cheaper than the check — the same trap as "verbs in another place" in t253.
- **The single row exposed a unit mismatch.** The instance field of a public parameter and the limits in the author's window showed the fraction as a raw number, while the same field inside a composition showed a percentage. After the move to the common row there is one unit; t691 and t692b were rewritten with an explanation.
- **The live check cannot be judged under build load.** `finish.sh` runs the scenarios while compilation is going on nearby: under such load 12 scenarios failed. The same set on a quiet machine (load average 3.6) gave 0 failures — this is a known flake under load, not a regression; a conclusion about the live scenarios is drawn ONLY from a separate run of `./scripts/live-check.sh`.
- **The first line of a test's debug output is glued to the test name** (`test foo ... FORM …`), and `grep "^FORM"` does not see it — half an hour went into searching for a "missing" window that was in fact processed first.

## Migration Plan

The project format does not change; the state of the groupings is the window's memory. Rolling back any re-layout is local: the form contract and the guard do not depend on how exactly a particular window is laid out.
