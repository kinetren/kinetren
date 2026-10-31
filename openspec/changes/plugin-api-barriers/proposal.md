# Proposal

## Why

`plugin-api-stage1` moved every user-facing effect out of the core except the twelve whose
position is a BARRIER (blur, glow, edges, vhs, sharpen, pixelate, halftone, radial_blur,
drop_shadow, chromatic_aberration, plus noise and greenscreen). The owner asked for "ALL the
effects out — a mechanism as flexible and extensible as possible" (2026-10-30) and for the
barriers to be a change of their own. They stayed because the plugin API has no contract for what
makes them barriers: a multi-pass GPU graph (mip down/up, a final pass), an integer CPU kernel with
a byte-equal AVX2 twin (t198), the K× resolution rule and a scratch pool. While they are enum
variants, an edit of one of them still rebuilds kt-model and every crate above it (t977 measured
the leaf rebuild only for the moved kinds), and a third-party barrier effect is impossible.

## What Changes

- **A barrier contract in `kt-plugin-api`:** `BarrierKernel` carries:
  - the CPU kernel over a frame, with the `res_scale` K, the burst index and a host scratch pool;
  - an optional SIMD twin, which the host gates by runtime detection and `KT_SIMD`;
  - a declared alpha relation;
  - a GPU PASS GRAPH: host nodes (`Resample` to a size rule, `Premul`, `Unpremul`) plus plugin
    fragment passes over `common.wgsl`, reading named slots (`Src`, `Prev`, `Saved(n)`), with
    16 uniform floats computed once on the CPU (f64 → f32).
- **The raster helpers every barrier shares move into the API:** bilinear resample, box mip, the
  integer luma and `noise_hash`/`noise_seed`. kt-fx and kt-gpu call the same text.
- **The twelve kinds move** into family crates: `fx-video-blur` (blur, glow, sharpen,
  radial_blur, drop_shadow), `fx-video-stylize` (edges, vhs, noise, pixelate, halftone,
  chromatic_aberration) and `fx-video-color` (greenscreen).
  - Each kind's file text, CPU bytes, GPU tolerance class and SIMD parity stay as they are.
  - Noise and greenscreen declare both a positioned tap (the fused position, D11.1) and a barrier
    kernel; the planner picks by position, as it does today.
- **kt-gpu assembles barrier pipelines from the registry**, as it already does for point-wise
  taps. The `Pipe`/`FxOp` arms of the twelve kinds and their WGSL files leave kt-gpu.
- **`FxInstance` keeps only** `Geometry`, `CamMotion`, `LayerMix`, `Plugin` and `Missing`;
  `apply_barrier`'s kind arms and the barrier kernels leave kt-fx.
- **Not in scope:** external hosts (wasm, dlopen/OFX: stages 2 and 3 of F-EXT-003). A third-party
  barrier plugin is possible only as a built-in crate.

## Capabilities

### New Capabilities
<!-- none: the `plugins` capability is created by plugin-api-stage1; this change adds to it -->

### Modified Capabilities
- `plugins`: a barrier kernel is a CPU kernel, an optional byte-equal SIMD twin and a declared GPU
  pass graph; the host gates the twin and refuses an inconsistent graph at registration.
- `video-fx`: the K× barrier requirement now holds for barrier PLUGINS. The K rule, the invariance
  check and the decode-scale seeding stay host rules applied to every registered barrier, with no
  per-kind list.

## Impact

- **Code:**
  - `src/libs/kt-plugin-api` (new `barrier` and `raster` modules).
  - `src/libs/kt-fx/src/{video.rs,video_simd.rs,geom.rs}`: the kernels leave, the planner reads
    manifests.
  - `src/libs/kt-gpu/src/engine.rs` and `shaders/`: the twelve pipes leave, a pass-graph executor
    arrives.
  - `src/libs/kt-model/src/project.rs`: twelve variants, their serde defaults and constructors.
  - New `src/plugins/fx-video-blur` and `src/plugins/fx-video-stylize`; `fx-video-color` grows.
  - `kt-plugins-builtin` lists the twelve ids.
- **Tests:**
  - These stay green unchanged: t26, t328, t181, t329, t198, t302–t304, t308, t332, t303, t973
    (frozen file forms) and t977.
  - New tests are reserved: t1051–t1056.
- **Depends on:** `plugin-api-stage1` archived first (the `plugins` spec exists).
- **Risk:** the AVX2 twins (`video_simd.rs`) are the largest move. A twin that diverges by one
  byte is caught by t198 running over the registry, not over a list.
