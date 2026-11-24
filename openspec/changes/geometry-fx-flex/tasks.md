# Tasks: geometry-fx-flex

> Decided 2026-11-20 (A24); revision 2 implementation-ready at `229c694b` (2026-11-24). Code starts on the orchestrator's go, after
> `preview-canvas-and-gizmos`. Tests: t3570–t3576, t4245.

## 0. Notes

- [x] 0.1 F-FX-019 (with the audit table); proposal, design (D0–D9), delta `video-fx`, `ui-reach`; `openspec validate geometry-fx-flex --strict`
- [x] 0.2 his answer Q44–Q45 (kkt-26.11.20 line 35), read against B6–B8

## 1. The steps (one commit: kt-plugin-api, `kt-fx/src/geom.rs`, `kt-gpu` packing and `geom_fused.wgsl`)

- [ ] 1.1 `GeoStep::{Reflect, Cells}`, primitives 6 and 7, `map_pos`, Const/PerPixel, `geom_specialized`; t3570, t3572, t3573, t3575
      (with their cases in t181, t224-class parity lists and t306's slices)
- [ ] 1.2 old slots: t297, t943, t944, t26 green; t3571

## 2. The plugins (`src/plugins/fx-video-distort`)

- [ ] 2.1 Mirror and EventGrid rows, `tile`, templates, variants and picker cells (t543); the catalog of kinds (t48, t322, t329); i18n ×5
- [ ] 2.2 the next free file version by content; t4245
- [ ] 2.3 the acceptance chain's stored frames; t3574

## 3. The interface

- [ ] 3.1 `HandleDecl::Rect` in `viewer.rs`'s handles; Mirror's and Tile's declarations; t3576; live `geomctl` gets a Tile step, live
      `gpufx` the chain

## 4. Close

- [ ] 4.1 What's new ("Mirror reflects across any line, EventGrid flips and slides its cells, Tile repeats a part of the picture and moves
      it"); F-FX-019 verdict; B6–B8 and A24 moved; the audit rows into F-FX-007; kt-fx and kt-gpu `CLAUDE.md` (the two primitives)
- [ ] 4.2 the cost of the chain against the old grid + mirror on llvmpipe in the report; `finish.sh` (orchestrator)
