# Tasks: geometry-fx-flex

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3570–t3584.

## 0. Notes first

- [x] 0.1 note F-FX-019 (`docs/features/fx/distort/mirror-grid-tile.md`)
- [x] 0.2 proposal, design (D1–D8, the audit table, class walk), delta `video-fx`; `openspec validate geometry-fx-flex --strict`
- [ ] 0.3 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The steps (one commit pair: kt-fx `geom.rs` ↔ kt-gpu `geom_fused.wgsl`)

- [ ] 1.1 `GeoStep::Reflect`, the Grid's flips/gap/phase/row shift, `GeoStep::Tile`; Jacobians; the uniform budget; t3570, t3572, t3573, t3575
- [ ] 1.2 old Mirror flips and EventGrids unchanged; t3571

## 2. The plugins (`src/plugins/fx-video-distort`)

- [ ] 2.1 Mirror rows and picker variants, EventGrid rows, the `tile` plugin; format by content
- [ ] 2.2 the owner's chain as stored reference frames; t3574

## 3. The interface

- [ ] 3.1 the preview crosshair and angle handle for Mirror, the source frame for Tile; i18n ×5; t3576

## 4. Close

- [ ] 4.1 What's new; F-FX-019 verdict → `CAPABILITY: openspec/specs/video-fx/`; registry rows B6–B8 moved; the audit rows into F-FX-007 per Q2
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
