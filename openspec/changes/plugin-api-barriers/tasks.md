# Tasks

Tests t1051–t1056 are reserved by this change. It starts after `plugin-api-stage1` is archived
(the `plugins` spec exists). Every stage ends green and is committed at once (owner's rule); each
moved kind's `FxOp`/`Pipe` arms, WGSL file, serde defaults and constructor are deleted in the
stage that moves it.

## 1. Freeze before any code moves

- [ ] 1.1 Write t1051 (`src/libs/kt-fx/tests/barrier_plugin.rs`): a CPU hash table of the twelve barrier kinds. Rows: the default and every built-in variant, at K=1 and K=2, on a frame with and without alpha. Green on the core variants. Red proof: perturb one kernel (blur's radius +1) and see its rows go red
- [ ] 1.2 Extend t973 with the twelve kinds' frozen file forms (default and a non-default form each, the `skip_serializing_if` fields absent at their defaults)
- [ ] 1.3 Record the t977 numbers for an edit in `kt-fx/src/video.rs` (where the barriers live now) next to stage 1's numbers in this file

## 2. The contract and the pilot (pixelate)

- [ ] 2.1 `kt_plugin_api::barrier`:
  - `BarrierKernel` with `cook`/`frame`/`frame_simd`/`alpha`/`gpu`, plus `BarrierFrame` and `BarrierCtx` (K, burst index, scratch, `&dyn RasterHost`);
  - `PassGraph` with its nodes `Resample{Size}`/`Premul`/`Unpremul`/`Save(n)`/`Pass{wgsl, entry, inputs, uniforms}`;
  - `Decl.class = Barrier`.

  Verify: the API crate builds without kt-fx
- [ ] 2.2 The registration refusals of design D5. Write t1054 first, red with the checks removed; verify t972 green
- [ ] 2.3 kt-fx:
  - implement `RasterHost` (`resample` = `bilinear_into` + its AVX2 twin, `mip_down`/`mip_up`, `luma`, `noise_hash`/`noise_seed`);
  - `apply_barrier` dispatches a plugin barrier through the kernel;
  - the host gates `frame_simd` by the one `KT_SIMD`/detection switch.

  Verify: t198 green
- [ ] 2.4 kt-gpu: a pass-graph executor (each distinct WGSL compiled once per engine over `common.wgsl`; slots counted from the graph; the existing `debug_assert` on the arena). Verify on the pilot
- [ ] 2.5 Move pixelate into `src/plugins/fx-video-stylize` (new crate; added to `kt-plugins-builtin` and `BUILTIN_IDS`, in its `VIDEO_ORDER` place). Verify t1051, t973, t26, t328, t181, t329, t332, t303, t977 green; commit stage 2

## 3. The mip kinds

- [ ] 3.1 Glow into `src/plugins/fx-video-blur` (new crate): premul around the mip on alpha (R.53 item 418), its AVX2 add twin through `frame_simd`. Verify t1051, t198, t181, t329, t303, the glow mip tests
- [ ] 3.2 Sharpen and drop_shadow into `fx-video-blur`, with the three-pass graphs. Verify t1051, t332, t181, t329, t303
- [ ] 3.3 Blur and radial_blur into `fx-video-blur`. Verify t1051, t181, t329, t303, t302; commit stage 3

## 4. The rest and the dual-position kinds

- [ ] 4.1 Edges (the luma + sobel twins), halftone and chromatic_aberration into `fx-video-stylize`. Verify t1051, t198, t332, t181, t329
- [ ] 4.2 Vhs and noise into `fx-video-stylize`, greenscreen into `fx-video-color`:
  - the burst index arrives through `BarrierCtx`;
  - noise and greenscreen keep their positioned tap (D11.1) next to the barrier kernel, and the planner chooses by position (design D4).

  Verify t1051, t198, t26, t302, t304, t308, t181, t329, t992
- [ ] 4.3 Write t1055 (`src/libs/kt-fx/tests/geo_fuse.rs`): a test barrier plugin the host does not name gets K=2 between a grid and a zoom and after S=2, as a built-in. Red with the planner reading K from a kind list
- [ ] 4.4 Write t1052 (`src/libs/kt-gpu/tests/parity.rs`): every registered barrier on the GPU within its class, and a two-pass test plugin needing no host code. Red with the saved slot ignored. Write t1053 (`src/libs/kt-fx/tests/simd_parity.rs`): the twin parity over the registry. Red with one twin perturbed; commit stage 4

## 5. Cleanup and acceptance

- [ ] 5.1 `FxInstance` keeps Geometry, CamMotion, LayerMix, Plugin, Missing. `apply_barrier` and kt-gpu's `FxOp` name no effect kind; the barrier `Pipe`s and WGSL files are gone. Write t1056 (`src/libs/kt-model/tests/plugin_format.rs`), red with a variant left. Verify `cargo test --workspace` green
- [ ] 5.2 Run `python3 src/scripts/plugin-rebuild-check.py check src/plugins/fx-video-blur/src/lib.rs`: no core library recompiled. Record the numbers here next to 1.3
- [ ] 5.3 Update `src/libs/kt-fx/CLAUDE.md` and `src/libs/kt-gpu/CLAUDE.md` (the "add a video effect" checklist for a barrier), `docs/architecture.md` and the root `CLAUDE.md` (no barrier kinds left in the core); commit stage 5

## 6. Finish

- [ ] 6.1 Run `./scripts/finish.sh --rebuild-check` from `src/`; record the version and the t977 numbers in the report
