# Tasks: preview-canvas-and-gizmos

> DESIGN ONLY until the owner answers Q1–Q2. Tests: t3597–t3599.

## 0. Notes

- [x] 0.1 note F-UI-011 (`docs/features/ui/viewer/preview-zoom-and-effect-handles.md`)
- [x] 0.2 proposal, design (D1–D5, class walk), deltas `ui-timeline`, `ui-reach`; `openspec validate preview-canvas-and-gizmos --strict`

## 1. The viewer (kt-app)

- [ ] 1.1 `Viewer` transform, zoom steps, pan, Ctrl+wheel / Ctrl+0 / middle and Space drag rows in `controls.rs`; the pasteboard; t3597
- [ ] 1.2 the frame, crosshairs and mask vertices take the viewer transform; no recompose on zoom; t3599

## 2. Handle kinds

- [ ] 2.1 kt-plugin-api `ParamSpec.handle`; declarations in the distort, blur, color (vignette) and three-d (corner pin) families
- [ ] 2.2 kt-app radius, angle and quad handles; one history step; keys on curved rows; t3598

## 3. Close

- [ ] 3.1 i18n ×5; planned live scenario `viewer_zoom` (written with the scenario); What's new; F-UI-011 verdict; registry B9 moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report
