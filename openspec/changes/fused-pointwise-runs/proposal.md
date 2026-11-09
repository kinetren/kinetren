# Proposal: fused-pointwise-runs

## Why

`docs/engine-audit.md` §3 ("Shaders: fused point-wise runs") and row 10 of its order, mechanism M7;
the feature note `docs/features/plugins/kernels/fused-kernels.md` (F-EXT-005). A chain of point-wise
effects outside a geometry series (Color Grade, Posterize, Duotone, Vignette one after another) runs
as one GPU pass per effect: each pass reads the whole frame from memory and writes it back, a
dispatch, a bind group and a pool texture each. gpu-pass-cost D2 measured 2–3 ms per such pass at
1080p on llvmpipe; the device-independent count is 8 bytes moved per pixel per effect. The owner's
standing go of 2026-11-09 ("когда эти закончат — продолжай другие делать по таблице", registry
`docs/ktprompts/kkt-26.11.09-index.md` I10) takes the table's next row; on fusion he agreed (H16,
`kkt-26.11.08`) that the stored frames may move by at most 1 LSB per former boundary.

## What Changes

- **The class walked** (design D1): every chain stage of every video family and the core nodes,
  sorted into "fuses in a point-wise run" and "ends a run", with the reason.
- **The plan names point-wise runs.** `plan_chain` (the one place of chain decisions) emits
  `PlanOp::PointRun { i, j, res_scale }` for two or more consecutive fusable point-wise stages at a
  barrier position; a geometry step, a barrier, a stop (layer strength, mask), and a point-wise
  plugin whose barrier position is a kernel of its own (noise, greenscreen) end it. A lone stage
  stays a `Barrier` op as before.
- **The plugin contract says which stage fuses** (additive, `kt-plugin-api`): `fuses_in_runs(plugin)`
  — a video plugin of class `PointWise` with a `tap_<id>` WGSL and no barrier kernel. The registry
  refuses a point-wise WGSL that does not define `fn tap_<id>(` or that reads a texture (a
  neighbourhood is not point-wise). No plugin is guessed: the class is declared and checked.
- **One generated WGSL pass per run shape** (kt-gpu): load once, the stages' `tap_<id>` functions in
  chain order with the same per-stage rounding the own passes apply, store once. The text is built
  from the run's shape (the sequence of plugin ids), compiled and warmed on the device's warm worker
  (pipeline-prewarm's rules: a frame never compiles, the worker holds `exit::busy()`), kept by shape,
  and in the disk pipeline cache. Until a shape is warm a frame runs the own passes: the same bytes.
- **The cap and the switch are measured at run time**: a run longer than the uniform's room (32
  stages) is cut into pieces of the room; a warm shape is tried against its own passes by interleaved
  frames, timed by gpu-pass-cost's pass timer, and a fused pass slower than its parts on this device
  is split in two halves, each tried again. Without timestamps on the device the fused pass stands
  (it moves fewer bytes on every device). No constant tuned to one machine.
- **Bytes**: the fused pass rounds and clamps between stages exactly as the former boundary did, so
  the fused and the unfused forms give the same bytes (measured on llvmpipe over every pair of
  point-wise plugins and a long run); the contract is the owner's ≤ 1 LSB per former boundary.
- **Prewarm**: the player hands the project's run shapes to the executor with its effect ids.
- **Instrument**: `kt-cli fuse-cost` — bytes moved and device time per frame of a point-wise chain,
  fused against unfused, interleaved.

## Impact

- `kt-plugin-api`: `fuses_in_runs`, two refusals in `Registry::register`.
- `kt-fx`: `PlanOp::PointRun`, `plan::run_shapes` (what prewarm reaches).
- `kt-gpu`: `FxOp::TapRun`, the generated text (`run_text`), run shapes in `Compiled`, the trial by
  the pass timer, `GpuFxEngine::{fuse_runs, run_shapes_ready, prewarm_runs}`.
- `kt-engine`: `VideoGpuExec::prewarm_runs`, the player's reach carries run shapes.
- `kt-cli`: `fuse-cost`.
- Capabilities: `video-plan`, `plugins`. Tests t2190–t2209.
