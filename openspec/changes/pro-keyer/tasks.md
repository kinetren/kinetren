# Tasks: pro-keyer

> Decided 2026-11-20 (A22: Q46 a, Q47 a); revision 2 implementation-ready at `fc60a5d9` (2026-11-25). After `chroma-key-upgrade`. Tests: t3585–t3590, t4254.

## 0. Notes

- [x] 0.1 F-MSK-009; proposal, design (D0–D10), deltas `video-fx`, `plugins`, `ui-reach`; `openspec validate pro-keyer --strict`
- [x] 0.2 his answer (kkt-26.11.20 line 33)

## 1. The held frame (kt-plugin-api, kt-engine, kt-gpu)

- [ ] 1.1 `BarrierKernel::held`, `Slot::Held`, `held_keys` in every asker, `LayerSrc.held`, `moment_key`; t3588, t4254

## 2. The kernel (`src/plugins/fx-video-color`)

- [ ] 2.1 linear decode and pack, the plate (pyramid or held), screen-ness, core, edge band, combine; t3585
- [ ] 2.2 shadow, spill and bounce with the three replacements; t3586
- [ ] 2.3 stored frames, preview = export, the Viewer's stages never rendered; t3587; the pass costs at 1080p in the report (card, llvmpipe)

## 3. The interface (kt-app)

- [ ] 3.1 the pipeline panel (a file of its own beside `isp_panel.rs`, one shared strip widget), stage eyes, Analyze, "Use this frame as the plate"; i18n ×5; t3589
- [ ] 3.2 live `pro_keyer` with a screenshot per step (t3590); idle with the keyer on a paused frame

## 4. Close

- [ ] 4.1 What's new ("a professional keyer: uneven screens, hair, shadows kept, green light taken off the subject"); F-MSK-009 verdict; B4, A22 moved
- [ ] 4.2 `finish.sh` (orchestrator)
