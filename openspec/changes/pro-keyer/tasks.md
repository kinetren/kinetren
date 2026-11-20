# Tasks: pro-keyer

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`); after `chroma-key-upgrade`. Tests reserved:
> t3585–t3599.

## 0. Notes first

- [x] 0.1 note F-MSK-009 (`docs/features/masks/keying/professional-keyer.md`)
- [x] 0.2 proposal, design (D1–D10, class walk), deltas `video-fx`, `plugins`, `ui-reach`; `openspec validate pro-keyer --strict`
- [ ] 0.3 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The held frame (kt-plugin-api, kt-engine)

- [ ] 1.1 `BarrierKernel::held`, `clip_frame_keys` returns it, chunk budget, layer key; t3588

## 2. The kernel (`src/plugins/fx-video-color`, kt-gpu)

- [ ] 2.1 linearize, plate pyramid (generated) or held plate, screen-ness, core (clip, choke, fill), edge band, combine; t3585
- [ ] 2.2 shadow ratio, spill/bounce removal with restore and replace modes; t3586
- [ ] 2.3 stored reference frames; preview = export; views preview-only; t3587

## 3. The interface (kt-app)

- [ ] 3.1 the pipeline panel, stage eyes → Viewer, Analyze, "Use this frame as the plate"; i18n ×5; t3589
- [ ] 3.2 planned live scenario `pro_keyer` written with the scenario, t3590; idle check with the keyer on a paused frame

## 4. Close

- [ ] 4.1 What's new; F-MSK-009 verdict → `CAPABILITY: openspec/specs/video-fx/`; registry row B4 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
