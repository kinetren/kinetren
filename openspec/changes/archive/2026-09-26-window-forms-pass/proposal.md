# Proposal

## Why

In R.64 the owner accepted the look of the "Transform" form and immediately said that this was not enough: "and don't forget that in the future, in all the other widgets and windows too, it will have to be gone through". While prose rather than a guard held this rule, the program accumulated exactly the defects that it protects against:

- **Body scrolling exists for exactly ONE of the twenty-one windows** (the track window, and even that appeared in R.65 only because test t775 failed on the "Close" and "Cancel" buttons hidden behind the edge). The FX window, morpher, transition, camera, project settings, render, envelope insertion and the three publishing windows have no scrolling at all — the same defect there awaits a person with a laptop.
- **Numbers are drawn outside the common widget**: their own `DragValue`s with their own limits and units sit in the morpher (4), the FX window (4), the timeline (3), the camera (3), automation (1). This is a second source of truth about limits — exactly what made Gain −24…24 in one window and −48…24 in another (R.33).
- **The deferred limit of R.64**: on the owner's project the transform frame comes out at 45×25 px (the "as is" rule plus a placeholder instead of unavailable media), and the eight handles cover it entirely — there is nothing to grab for the body.

## What Changes

- **A form contract for a window** — one for all windows of the `WINDOWS` registry: the body scrolls, and the action buttons ("Close", "Cancel", "Apply", "Render") live OUTSIDE the scroll and are reachable at any screen height; the numeric field is drawn by the common row of widgets; the limits, step and unit are taken from the parameter's passport, and for quantities without a passport they are declared in ONE place next to their definition, not in the drawing code.
- **A guard on all 21 windows**: the kt-app source scanner fails on `egui::DragValue::new` outside `widgets.rs`, and a harness test opens EVERY window of the registry in a low window and requires that its action buttons remain within the screen bounds and can be pressed.
- **Re-layout of the six densest forms** on the model of "Transform" (collapsible groups, pairs on one row, units, a slider for a bounded quantity, a reset with one action): the FX window, morpher, transition, camera, project settings, render.
- **The handles of a small frame are moved outward**: when the transform frame is smaller than three grab zones on a side, the eight scale handles are drawn and caught OUTSIDE the edges of the frame, while its body remains under the carry-over; the anchor and the rotation handle stay in their places.
- NOT included: the re-layout of the other fifteen windows (they fall under the guard only), a single "inspector" instead of windows, a change to the set of parameters, new windows (the R.63 ban), themes and colors.

## Capabilities

### New Capabilities

(none — all the work falls on existing capabilities)

### Modified Capabilities

- `ui-reach`: to the requirement on the window registry is added a FORM CONTRACT (body scrolling, reachable action buttons, numbers only through the common widget, limits and units from the passport) and a guard that checks it on all windows of the registry.
- `ui-timeline`: the transform frame at a small size moves its scale handles outside its edges, keeping the body under the carry-over; the "Transform" form remains the model to which the six dense windows are brought.

## Impact

- `src/apps/kt-app/src/widgets.rs` — the common building blocks of the form: the pair row, the group, the number row WITHOUT a passport (unit and limit as arguments), so that the windows have something to replace their own `DragValue`s with.
- `src/apps/kt-app/src/{fx_editor,morpher_editor,transition_editor,camera_editor,settings_window,render_dialog}.rs` — body scrolling, buttons outside it, re-layout into groups and pairs.
- The other windows (`envclip`, `pubparam` ×3, modals in `app.rs`) — only scrolling and buttons; their forms are not touched.
- `src/apps/kt-app/src/reach_tests.rs` — the guard of the form contract over the `WINDOWS` registry.
- `src/apps/kt-app/src/geomctl.rs` + `src/libs/kt-fx/src/geom.rs` — moving the handles of a small frame outward (the screen geometry of the handles and their hit test).
- Tests: t779+ (the guard, per-window checks, the small frame); the live scenario `src/scripts/live/forms.py`.
