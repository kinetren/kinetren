# Tasks: temporal-barrier-3dnr

> Decided 2026-11-20 (A21); revision 2 implementation-ready at `fc60a5d9` (2026-11-25). Events need nothing unmerged; a track's camera waits for
> `track-stream-stage`. Tests: t3545–t3551, t4253.

## 0. Notes

- [x] 0.1 F-FX-017; proposal, design (D0–D8), deltas `plugins`, `video-plan`, `video-fx`; `openspec validate temporal-barrier-3dnr --strict`
- [x] 0.2 his answers Q36–Q37 (kkt-26.11.20 line 32)

## 1. The hooks (kt-plugin-api, kt-model, kt-engine)

- [ ] 1.1 `StreamKernel::{active, split}`, `StreamLayout::continues`, `HeldStreamOut::Rgba` and its pass in kt-gpu; `is_stream` asks `active`; t3545; Datamosh's tests unchanged
- [ ] 1.2 `StreamStage` with a split slot: the front part joins `front`, the back part takes the place in `after_chain` (masks, bypass, graph arms); t3546, t4253

## 2. The camera (`plugins/fx-video-camera`)

- [ ] 2.1 `camera_isp_front`/`_back`/`_scene` from today's `gpu_values` by pass ranges: with everything off the bytes are the barrier's (the stored frames)
- [ ] 2.2 the recursion's compute job, checkpoints, keys; t3547, t3548, t3549
- [ ] 2.3 rolling shutter and flicker; t3550
- [ ] 2.4 rows, presets, the motion map, the panel's blocks; i18n ×5; the next free file version

## 3. Close

- [ ] 3.1 t3551 (`idle`); live `camera` gets a TNR step and the panel's
- [ ] 3.2 What's new ("the camera's temporal denoiser with its trails, rolling shutter and mains flicker"); F-FX-017 verdict; I1, A21 moved; kt-engine and camera `CLAUDE.md`
- [ ] 3.3 the seek's cost on llvmpipe in the report; `finish.sh` (orchestrator)
