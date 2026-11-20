# Tasks: temporal-barrier-3dnr

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3545–t3557.

## 0. Notes first

- [x] 0.1 note F-FX-017 (`docs/features/fx/simulate/camera-temporal-nr.md`)
- [x] 0.2 proposal, design (D1–D9, physics first, class walk), deltas `plugins`, `video-plan`, `video-fx`; `openspec validate temporal-barrier-3dnr --strict`
- [ ] 0.3 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The contract (kt-plugin-api)

- [ ] 1.1 `VideoClass::Temporal`, `TemporalKernel` (warm, grid, pre, step, post, key), registry refusals; t3545

## 2. The host (kt-engine, kt-gpu)

- [ ] 2.1 `StreamStage::Temporal`: anchors, warm-up, checkpoints in the pinned tier, `needs`, front memo reuse; t3546, t3547
- [ ] 2.2 kt-gpu state textures (`R32Uint` words), the step graph on the compute host; ledger rows

## 3. The camera (`src/plugins/fx-video-camera`)

- [ ] 3.1 the ISP split into pre / step / post when a temporal row is on; rows, preset, motion-map view; t3548, t3549
- [ ] 3.2 rolling shutter (W = 1) and flicker (row time uniform); t3550
- [ ] 3.3 the ISP window's TNR block (device-like panel)

## 4. Close

- [ ] 4.1 format by content; idle scene; t3551
- [ ] 4.2 What's new; F-FX-017 verdict → `CAPABILITY: openspec/specs/video-fx/`; registry row I1 moved
- [ ] 4.3 `src/scripts/finish.sh` (orchestrator), version in the report
