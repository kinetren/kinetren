# Tasks: preview-canvas-and-gizmos

> DESIGN ONLY until the orchestrator's go; no open question. Revision 2. Tests: t3597–t3599, t4229–t4231.
> Areas: PREVIEW-UI (`app.rs` `draw_preview`, `geomctl.rs`, `maskctl.rs`, `centerctl.rs`), `controls.rs` (appends command
> 38: another change appending a command takes the next number), kt-plugin-api `UiDecl`. Needs nothing unmerged.

## 0. Notes

- [x] 0.1 F-UI-011; proposal, design (D0–D6), deltas `ui-timeline`, `ui-reach`, `ui-controls`; strict
- [x] 0.2 A26 read; the keys, rows and zones corrected against the code (D0)

## 1. The viewer

- [ ] 1.1 `viewer.rs`, `draw_preview` through it, gestures, the corner menu, `Cmd::PreviewFit`; t3597, t4229, t1702 (the settings code keeps old codes)
- [ ] 1.2 the zones of D2; the title's layout once; t3599, t4230; t495, t760, t760b, t1282 unchanged

## 2. Handles

- [ ] 2.1 `HandleDecl` in `UiDecl`, the checks of `register`, the six declarations; t4231
- [ ] 2.2 `handlectl.rs` over the three owners; t3598; the centre's tests and live `fx_center` unchanged

## 3. Close

- [ ] 3.1 i18n ×5; live scenario `viewer_zoom` (a step and a screenshot: Geometry zoom 3, Ctrl+wheel to 33 %, a corner, the rotation, the pan, Shift+0, a corner pin); `docs/guide/shortcuts.md`
- [ ] 3.2 the load check: paused on the heavy showcase at 33 % with the mouse moving, an effects window with a Twirl selected
- [ ] 3.3 What's new; F-UI-011 verdict; registry B9, A26
- [ ] 3.4 `src/scripts/finish.sh` (orchestrator), version in the report
