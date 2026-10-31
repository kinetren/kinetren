# Tasks

Tests t1051–t1057 are reserved by this change (t1057 for a defect found in stage 3). It starts after `plugin-api-stage1` is archived
(the `plugins` spec exists). Every stage ends green and is committed at once (owner's rule); each
moved kind's `FxOp`/`Pipe` arms, WGSL file, serde defaults and constructor are deleted in the
stage that moves it.

## 1. Freeze before any code moves

- [x] 1.1 Write t1051 (`src/libs/kt-fx/tests/barrier_plugin.rs`): a CPU hash table of the twelve barrier kinds. Rows: the default and every built-in variant, at K=1 and K=2, on a frame with and without alpha. Green on the core variants. Red proof: perturb one kernel (blur's radius +1) and see its rows go red
  - 144 rows (12 kinds × 3 file forms × K∈{1,2} × opaque/alpha), each kind built from its FILE
    text, so the same line reads as the variant now and the plugin slot later; 50×30 and 100×60
    (AVX2 tails). Red with blur's radius +1, as asked.
  - Found while freezing, not fixed here (a fix moves bytes, so it is the owner's decision): noise
    at its file default `amount` 0.08 is an IDENTITY at the barrier position. Its k = ⌊0.08·32⌋ = 2,
    and the offset `n·k/32767` with |n| ≤ 16384 truncates to 0 on every pixel but n = −16384.
    kt-app's "Noise" template is `noise(0.08)`, so a person who adds Noise sees nothing until they
    raise the amount; the registry's `all()` template uses 0.3. The fused tap has the same formula.
- [x] 1.2 Extend t973 with the twelve kinds' frozen file forms (default and a non-default form each, the `skip_serializing_if` fields absent at their defaults)
  - `t973_barrier_kinds_keep_their_text` (`kt-model/tests/plugin_format.rs`): 24 file lines (each
    kind's empty form → the frozen default text, and a form that moves every row) read and written
    again equal the text printed by the variants.
- [x] 1.3 Record the t977 numbers for an edit in `kt-fx/src/video.rs` (where the barriers live now) next to stage 1's numbers in this file
  - 2026-10-31, 32 cores, dev profile, warm tree:
    | edit | `build -p kt-app -p kt-cli` | `test --workspace --no-run` | load before | libraries/binaries again |
    |---|---|---|---|---|
    | `kt-fx/src/video.rs` (the barriers now) | 19.1 s | 40.4 s (tests of 5 packages) | 1.3 | kt-fx, kt-engine, kt-gpu, kt-app, kt-cli |
    | the pilot plugin `fx-video-color` | 5.6 s | 157.9 s (tests of 10 packages) | 14.6 | fx-video-color, kt-plugins-builtin, kt-app, kt-cli |
  - The plugin edit's binaries are cheaper. Its TEST build costs ~4× an edit inside kt-fx, because
    `kt-plugins-builtin` is a dev-dependency of the tests of 10 packages (kt-model's, kt-media's,
    kt-otio's too), while a kt-fx edit reaches 5. Two runs agree (158 s here, 167 s at the archive
    of plugin-api-stage1), so it is not noise. Moving the barriers will give their edits the same
    cost; the proposed follow-up "Cut the test-build cost of a plugin edit" is what removes it.

## 2. The contract and the pilot (pixelate)

- [x] 2.1 `kt_plugin_api::barrier`:
  - `BarrierKernel` with `cook`/`frame`/`shaders`/`gpu` (the SIMD twin a branch of `frame` on `cx.simd`), `Decl.alpha`, the host's `Frame` and `Scratch`, and `BarrierCtx` (K, burst index, simd, `&dyn RasterHost`);
  - `PassGraph` with its nodes `Resample{Size}`/`Premul`/`Unpremul`/`Save(n)`/`Pass{wgsl, entry, inputs, uniforms}`;
  - `Decl.class = Barrier`.

  Verify: the API crate builds without kt-fx
  - `barrier.rs`: `Frame` (kt-fx re-exports it), `Scratch`, `RasterHost` (resample, sample,
    premultiply/unpremultiply; `luma` waits for edges in 4.1), `BarrierCtx`, `AlphaRel`, `GpuCtx`,
    `Slot`, `Node`, `BarrierKernel`, `check_graph`, and `MAX_RES_SCALE` / `MAX_INTER_DIM`, which
    kt-fx now takes from here (with a const assert against `SS_MAX`). `Plugin::barrier()`,
    `Decl.alpha`, `Gpu::Barrier`.
- [x] 2.2 The registration refusals of design D5. Write t1054 first, red with the checks removed; verify t972 green
  - t1054 (`kt-plugin-api/tests/barrier_registry.rs`) refuses:
    - a barrier without a CPU kernel;
    - one without a GPU kernel;
    - a barrier offering a tap's WGSL;
    - `Gpu::Barrier` on a point-wise plugin;
    - a read of an unsaved slot;
    - a half-size output;
    - a resample above 4096;
    - a trailing `Save`;
    - a pass naming a missing shader;
    - an empty graph on a frame with alpha.

    It registers a graph of host nodes alone (blur's form). The graph is checked on the
    passport's defaults and top, at K=1 and K=4. Red with the CPU-kernel check removed.
  - "A missing alpha relation" is not representable: `AlphaRel` defaults to `Preserves`, the silent
    one, so the refusal is gone from the spec. A trailing `Save` IS refused: found while writing
    the executor — the saved list would release the texture the output still reads.
- [x] 2.3 kt-fx:
  - implement `RasterHost` (`resample` = `bilinear_into` + its AVX2 twin, `mip_down`/`mip_up`, `luma`, `noise_hash`/`noise_seed`);
  - `apply_barrier` dispatches a plugin barrier through the kernel;
  - the host sets `BarrierCtx.simd` from the one `KT_SIMD`/detection switch.

  Verify: t198 green
  - `kt_fx::video::{Raster, plugin_barrier}`; `FxScratch: Scratch`; `apply_barrier` makes ONE
    dynamic call into a barrier plugin, `simd` = `simd_level() == Avx2`.
- [x] 2.4 kt-gpu: a pass-graph executor (each distinct WGSL compiled once per engine over `common.wgsl`; slots counted from the graph; the existing `debug_assert` on the arena). Verify on the pilot
  - `plugin_barriers()` and `barrier_pipe_base`: a barrier's passes follow the point-wise plugins'
    own passes in `plugin_pipes`. `FxOp::PluginBarrier { base, nodes }`, whose `passes()` count is
    its non-`Save` nodes. `chain_ops` builds the graph against the frame here and applies
    `Decl.alpha` to the fold; the executor tracks `Src`/`Prev`/`Saved` and releases what nothing
    reads. `assembled_shaders` (t974) is untouched.
- [x] 2.5 Move pixelate into `src/plugins/fx-video-stylize` (new crate; added to `kt-plugins-builtin` and `BUILTIN_IDS`, in its `VIDEO_ORDER` place). Verify t1051, t973, t26, t328, t181, t329, t332, t303, t977 green; commit stage 2
  - The variant, its tag arm, passport, constructor and `default_twelve_u32` leave kt-model; its arm
    and kernel leave kt-fx; `Pipe::Pixelate`, `FxOp::Pixelate` and `pixelate.wgsl` (moved into the
    crate) leave kt-gpu; kt-app's name, template, meta and variant rows go to the manifest. Callers
    use `FxInstance::builtin("pixelate")` / `plugin_with`.
  - t1051, t973 (`t973_barrier_kinds_keep_their_text`), t26, t328 green; t329 holds pixelate at
    `FrameTol::EXACT` on the GPU, so the graph gives the variant's bytes. The workspace was 1273/1281
    under load; the 8 reds (t801, t795, t640, t952, t951, t815, t476, t411) are green alone — t640
    first red alone with "database is locked" while the machine was at load 40, then green.

## 3. The mip kinds

- [x] 3.1 Glow into `src/plugins/fx-video-blur` (new crate): premul around the mip on alpha (R.53 item 418), its AVX2 add twin behind `cx.simd`. Verify t1051, t198, t181, t329, t303, the glow mip tests
- [x] 3.2 Sharpen and drop_shadow into `fx-video-blur`, with the three-pass graphs. Verify t1051, t332, t181, t329, t303
- [x] 3.3 Blur and radial_blur into `fx-video-blur`. Verify t1051, t181, t329, t303, t302; commit stage 3
  - `src/plugins/fx-video-blur` holds the five kinds.
    - Each CPU kernel is the kt-fx kernel verbatim; the mip and the samples go through
      `RasterHost`, the same functions the core calls.
    - Glow's AVX2 add twin moved into the crate's `simd.rs`, behind `cx.simd`; t198 reaches it
      through `fx_video_blur::simd_testing`.
    - The GPU graphs repeat the former `FxOp` arms node for node: blur is host nodes only;
      drop_shadow's graph is empty when the fold KNOWS the frame is opaque.
  - kt-gpu lost:
    - `Pipe::{GlowAdd, SharpenAdd, RadialBlur, ShadowUnder}`, their WGSL (moved) and five
      `FxOp`s;
    - `FxOp::Premul`/`Unpremul`, now graph nodes.
  - kt-model lost the five variants, their passports, tags and no-argument constructors, plus ten
    serde defaults. `blur(r)` and `glow(s)` stay as parametric constructors over `plugin_with`.
  - kt-app: the tints and the spin checkbox are drawn by plugin id. `UiDecl.alpha_sample` replaces
    `needs_alpha`'s match on `DropShadow`, so the picker names no kind.
  - Verified:
    - t1051 (on the AVX2 path and with `KT_SIMD=0`), t973, t26, t328, t198
      (`KT_REQUIRE_SIMD=1`), t302, t303, t332;
    - kt-gpu 28/28 with `KT_REQUIRE_GPU=1` (t181, t329);
    - workspace 1279/1283, the four reds (t815, t951, t952, t802) green alone.
  - **Found and fixed, two defects plugin-api-stage1 left in "is this the same kind":**
    - `FxInstance::lerp` (the picker's strength axis) compared the enum discriminant. Every
      plugin slot shares it, so blur's `radius` blended into glow's (t550c went red when both
      moved). It now compares the tag.
    - `kt_fx::chain_fingerprint`, which decides whether `FxStateStore` keeps a clip's processor
      (invariant 3), hashed the discriminant only. Since stage 3 of plugin-api-stage1 an EQ
      replaced by a filter in the same slot kept the EQ's node, which received the filter's
      values until a seek. It now hashes the tag; t1057 (`kt-fx/tests/chain_kind.rs`) was red on
      the old code. The `audio-render` delta records it.
    - kt-app's "a variant keeps its kind" check compared the discriminant too; it compares the
      tag now.

## 4. The rest and the dual-position kinds

- [x] 4.1 Edges (the luma + sobel twins), halftone and chromatic_aberration into `fx-video-stylize`. Verify t1051, t198, t332, t181, t329
  - The three kernels are verbatim; their samples go through `RasterHost::sample`. Edges' `luma_avx2`
    and `sobel_row_avx2` moved into the crate's `simd.rs`, behind `cx.simd`, and t198 still checks
    them through the public chain. kt-gpu lost `Pipe::{Edges, ChromaAb, Halftone}`, their `FxOp`s
    and the three WGSL files (moved). kt-model lost the variants, their passports and constructors
    and five serde defaults; `edges(threshold)` stays parametric. kt-app draws edges' line colour by
    plugin id.
  - Verified: t1051 on both SIMD paths, t198 (`KT_REQUIRE_SIMD=1`), t26 scalar, kt-gpu 28/28 with
    `KT_REQUIRE_GPU=1`. The workspace was 1276/1283; the seven load reds (t795, t640, t952, t951,
    t815, t200, t194) are green alone.
- [x] 4.2 Vhs and noise into `fx-video-stylize`, greenscreen into `fx-video-color`:
  - the burst index arrives through `BarrierCtx`;
  - noise and greenscreen keep their positioned tap (D11.1) next to the barrier kernel, and the planner chooses by position (design D4).

  Verify t1051, t198, t26, t302, t304, t308, t181, t329, t992
  - Noise and greenscreen are POINT-WISE plugins with a barrier kernel. Inside a series their tap
    is the former fused arm verbatim (`tap_noise`/`tap_greenscreen` WGSL with their own hash
    names; the f64 `tap_at`/`tap`); at a series edge the integer kernel with K×, noise's AVX2 twin
    and its own pass.
  - API additions:
    - `TapCtx.burst_index` (noise's seed);
    - `VideoKernel::gpu_cooked` (GPU numbers from the cooked form);
    - `VideoKernel::active` (noise at k = 0 puts no tap in the program, as the core kind did);
    - `noise_hash`/`noise_seed` in the API (kt-fx re-exports them; `lowbias32` got `#[inline]`).
  - The fused program applies a plugin tap's `Decl.alpha` (greenscreen produces). kt-gpu's
    `plugin_barriers` and the registry's graph check include point-wise plugins with a barrier
    kernel.
  - kt-gpu lost:
    - `Pipe::{Noise, Greenscreen, Vhs}`, their `FxOp`s and WGSL (moved);
    - the fused pass's tap kinds 0/1 and its `nh`/`noff`.
  - kt-model lost the three variants, their passports and ten serde defaults; `noise(amount)`,
    `vhs(shift, bleed)` and `greenscreen(key, tol, soft)` stay parametric. kt-app draws the key
    colour and noise's colour flag by plugin id.
  - Verified:
    - t1051 on both SIMD paths, t198 (`KT_REQUIRE_SIMD=1`), t26 scalar;
    - kt-gpu 28/28 with `KT_REQUIRE_GPU=1` (t181, t329, t992);
    - the workspace: t801, t815, t951, t476 are load reds, green alone.
  - Two more "same kind by discriminant" checks went blind once every barrier was a plugin:
    - t824's "≥ 3 video FX kinds" in the showcase;
    - t550d's "the kind does not change along the strength axis".
    Both compare tags now. t974's tap list grows by greenscreen and noise.
- [ ] 4.3 Write t1055 (`src/libs/kt-fx/tests/geo_fuse.rs`): a test barrier plugin the host does not name gets K=2 between a grid and a zoom and after S=2, as a built-in. Red with the planner reading K from a kind list
- [ ] 4.4 Write t1052 (`src/libs/kt-gpu/tests/parity.rs`): every registered barrier on the GPU within its class, and a two-pass test plugin needing no host code. Red with the saved slot ignored. Write t1053 (`src/libs/kt-fx/tests/simd_parity.rs`): the twin parity over the registry. Red with one twin perturbed; commit stage 4

## 5. Cleanup and acceptance

- [ ] 5.1 `FxInstance` keeps Geometry, CamMotion, LayerMix, Plugin, Missing. `apply_barrier` and kt-gpu's `FxOp` name no effect kind; the barrier `Pipe`s and WGSL files are gone. Write t1056 (`src/libs/kt-model/tests/plugin_format.rs`), red with a variant left. Verify `cargo test --workspace` green
- [ ] 5.2 Run `python3 src/scripts/plugin-rebuild-check.py check src/plugins/fx-video-blur/src/lib.rs`: no core library recompiled. Record the numbers here next to 1.3
- [ ] 5.3 Update `src/libs/kt-fx/CLAUDE.md` and `src/libs/kt-gpu/CLAUDE.md` (the "add a video effect" checklist for a barrier), `docs/architecture.md` and the root `CLAUDE.md` (no barrier kinds left in the core); commit stage 5

## 6. Finish

- [ ] 6.1 Run `./scripts/finish.sh --rebuild-check` from `src/`; record the version and the t977 numbers in the report
