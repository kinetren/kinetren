# Tasks: magic-wand-mask

> DESIGN ONLY until the orchestrator's go; no open question. Revision 2. Tests: t3535–t3543 (t3544 spare).
> Areas: MASKS (kt-model `mask.rs`, kt-fx `mask.rs` + `wand.rs`, kt-engine `video.rs`), PREVIEW-UI (`maskctl.rs`,
> `masks.rs`, the menu). Needs nothing unmerged. `preview-canvas-and-gizmos` writes `maskctl.rs` too: one at a time.

## 0. Notes first

- [x] 0.1 F-MSK-008; proposal, design (D0–D7), deltas; strict; OTIO's planned row
- [x] 0.2 A20 folded in, the code's facts (polyline rasterizer, stream events, instances)

## 1. The model (kt-model `mask.rs`, `project_file.rs`)

- [ ] 1.1 `MaskShape::Wand`, rows by seed id, `check_masks`, `WandOnTrack`; every `match` on `MaskShape` (kt-model `mask.rs`, kt-app `maskctl.rs`: the only two files); t3538

## 2. The contours (kt-fx)

- [ ] 2.1 `contours_of` in `one_mask` for many contours; t1269 unchanged
- [ ] 2.2 `wand.rs`: D2; the memo; t3535, t3537, t3541

## 3. The plan (kt-engine)

- [ ] 3.1 `MaskOp.pixels`, `layer_src`, blending, `reads`, `ops_for`; the Debug for the keys; t3536, t3542
- [ ] 3.2 `leaf_keys` orders the source for a wand on a stream event; an instance skips; t3543

## 4. The interface (kt-app)

- [ ] 4.1 the tool, gestures, ring, list row, hints; i18n ×5; t3539
- [ ] 4.2 live scenario `wand` on our fixture (a screenshot per step), t3540; the `idle` scene with a wand on a paused frame

## 5. Close

- [ ] 5.1 What's new; F-MSK-008 → `CAPABILITY: openspec/specs/video-fx/`; registry G1, A20; the OTIO row dated
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report
