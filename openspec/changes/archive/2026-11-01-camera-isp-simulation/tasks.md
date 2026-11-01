# Tasks

Request I1 of kkt-26.11.01. Test numbers t1210–t1234 are reserved by this change. Every stage ends
green and is committed at once (English message, first line "camera-isp-simulation <stage>: …");
`python3 src/scripts/target-prune.py` after every committed stage.

## 0. Records before code

- [x] 0.1 F-FX-010 (`docs/features/fx/simulate/camera-sensor-and-isp.md`), README row and `fx/` 9 / 9; `features-index.py --check` green
- [x] 0.2 proposal, design, spec deltas (video-fx, plugins), tasks; `openspec validate camera-isp-simulation --strict` green

## 1. The values door (plugins)

- [x] 1.1 `BarrierKernel::frame_values`/`gpu_values` (defaulted); kt-fx `apply_barrier`, kt-gpu barrier op and the registry graph check call them; t1053's loop calls `frame_values`
- [x] 1.2 t1210 (`src/libs/kt-fx/tests/barrier_values.rs`): a twelve-row test barrier; blur's golden unchanged

## 2. The kernel (fx-video-camera)

- [x] 2.1 The crate, the passport (stages, bypasses, choices), the layout tables, the presets
- [x] 2.2 The CPU passes (P0–P5, extended to twelve in 2b.2) (f32, the D1 quantization points), the noise (D4), K× (D6), borders (D7), threads and the AVX2 twin (D8)
- [x] 2.3 The WGSL passes (twelve after 2b.2) and `gpu_values`
- [x] 2.4 Registration: workspace, `BUILTIN_IDS`, catalogs ×5 (names, keywords, rows, stages, choices, presets), F-FX-007's list, goldens (t1051's table), t1066 green
- [x] 2.5 t1211–t1216, t1218 (`src/plugins/fx-video-camera/tests/consequences.rs`)
- [x] 2.6 t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`) and t1052 with the new kind

## 2b. The owner's review (2026-11-01): unprocessing, the defect ↔ block pairs, the view

- [x] 2b.1 Remarks recorded in `docs/user-feedback.md`; the owner chose the real recursive 3DNR (GPU-accelerated) — it waits for `datamosh-codec-path` in master (the orchestrator told)
- [x] 2b.2 Twelve passes (design D1): unprocessing, lens with distortion/defocus/colour shading, pixel and readout (blooming, row/column noise), raw front (BLC, OB row/column, DPC, GEQ), raw NR by the noise model, IR/LSC/colour LSC/WB, demosaic + highlight handling, dewarp + CA correction, YUV with FCS and purple-fringe suppression, chroma, view; CPU, AVX2 twin, WGSL
- [x] 2b.3 t1220 (each defect ↔ its block), t1221 (view), t1222 (unprocessing), t1223 (raw NR, blooming)
- [x] 2b.4 3DNR with motion-detector trails, rolling shutter, flicker: MOVED to the change `temporal-barrier-3dnr` (the orchestrator, 2026-11-01: the owner asked for the GPU; the stream class is a CPU-only source stage) — design "NOT HERE"

## 3. The window

- [x] 3.1 The pipeline panel (stage blocks with bypass, the shared row widgets) in the effect window
- [x] 3.2 t1219 (`reach_tests.rs`, PATH), layout/i18n/font tests green; the CJK subset regenerated
- [x] 3.3 A live scenario with a screenshot per PATH step on our fixtures (`src/scripts/live/`)

## 4. Cost and finish

- [x] 4.1 The cost per 1080p frame (release, CPU scalar/AVX2, GPU) into design D9
- [x] 4.2 What's new line (kt26.11), registry row I1 closed, `ktprompts-index.py --check`
- [x] 4.3 `git merge master`; `cargo test --workspace` and `cargo clippy --all-targets` green; commit
- [ ] 4.4 `./scripts/finish.sh` from `src/`; the version recorded here
- [x] 4.5 Archive: specs merged, the "why" into video-fx/plugins design; F-FX-010 `done`
