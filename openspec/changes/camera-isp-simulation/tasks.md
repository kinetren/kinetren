# Tasks

Request I1 of kkt-26.11.01. Test numbers t1210–t1234 are reserved by this change. Every stage ends
green and is committed at once (English message, first line "camera-isp-simulation <stage>: …");
`python3 src/scripts/target-prune.py` after every committed stage.

## 0. Records before code

- [x] 0.1 F-FX-010 (`docs/features/fx/simulate/camera-sensor-and-isp.md`), README row and `fx/` 9 / 9; `features-index.py --check` green
- [x] 0.2 proposal, design, spec deltas (video-fx, plugins), tasks; `openspec validate camera-isp-simulation --strict` green

## 1. The values door (plugins)

- [ ] 1.1 `BarrierKernel::frame_values`/`gpu_values` (defaulted); kt-fx `apply_barrier`, kt-gpu barrier op and the registry graph check call them; t1053's loop calls `frame_values`
- [ ] 1.2 t1210 (`src/libs/kt-fx/tests/barrier_values.rs`): a twelve-row test barrier; blur's golden unchanged

## 2. The kernel (fx-video-camera)

- [ ] 2.1 The crate, the passport (stages, bypasses, choices), the layout tables, the presets
- [ ] 2.2 The CPU passes P0–P5 (f32, the D1 quantization points), the noise (D4), K× (D6), borders (D7), threads and the AVX2 twin (D8)
- [ ] 2.3 The WGSL passes P0–P5 and `gpu_values`
- [ ] 2.4 Registration: workspace, `BUILTIN_IDS`, catalogs ×5 (names, keywords, rows, stages, choices, presets), F-FX-007's list, goldens (t1051's table), t1066 green
- [ ] 2.5 t1211–t1216, t1218 (`src/plugins/fx-video-camera/tests/consequences.rs`)
- [ ] 2.6 t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`) and t1052 with the new kind

## 3. The window

- [ ] 3.1 The pipeline panel (stage blocks with bypass, the shared row widgets) in the effect window
- [ ] 3.2 t1219 (`reach_tests.rs`, PATH), layout/i18n/font tests green; the CJK subset regenerated
- [ ] 3.3 A live scenario with a screenshot per PATH step on our fixtures (`src/scripts/live/`)

## 4. Cost and finish

- [ ] 4.1 The cost per 1080p frame (release, CPU scalar/AVX2, GPU) into design D9
- [ ] 4.2 What's new line (kt26.11), registry row I1 closed, `ktprompts-index.py --check`
- [ ] 4.3 `git merge master`; `cargo test --workspace` and `cargo clippy --all-targets` green; commit
- [ ] 4.4 `./scripts/finish.sh` from `src/`; the version recorded here
- [ ] 4.5 Archive: specs merged, the "why" into video-fx/plugins design; F-FX-010 `done`
