# Tasks: magic-wand-mask

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3535–t3544.

## 0. Notes first

- [x] 0.1 note F-MSK-008 (`docs/features/masks/shapes/magic-wand-mask.md`)
- [x] 0.2 proposal, design (D1–D7, class walk), deltas `video-fx`, `project-format`, `ui-reach`; `openspec validate magic-wand-mask --strict`
- [x] 0.3 planned row in "What OTIO export loses"
- [ ] 0.4 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The model and the rasterizer

- [ ] 1.1 kt-model `MaskShape::Wand`, seed rows by stable id, validation (no track wand); t3538
- [ ] 1.2 kt-fx `wand_coverage` (box downscale, distance, scanline fill, ramp, holes, chamfer grow, upscale); t3535, t3537

## 2. The plan (kt-engine)

- [ ] 2.1 `ClipMaskPlan` hands the source frame to a wand; seeds mapped through the leading Geometry; the memo keyed by content serial; t3536

## 3. The interface (kt-app)

- [ ] 3.1 preview menu, seed clicks, tolerance drag, form rows, "Link to tracker point…"; i18n ×5; t3539
- [ ] 3.2 planned live scenario `wand` written with its scenario, t3540; the `idle` scene with a wand on a paused frame

## 4. Close

- [ ] 4.1 What's new; F-MSK-008 verdict → `CAPABILITY: openspec/specs/video-fx/`
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
