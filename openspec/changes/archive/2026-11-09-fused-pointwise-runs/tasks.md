# Tasks: fused-pointwise-runs

- [x] 1.1 The class walk (design D1): every video family and the core nodes, fuses / ends a run, why
- [x] 2.1 kt-plugin-api: `fuses_in_runs`, the registry's refusals of an impure tap; t2194
- [x] 2.2 kt-fx: `PlanOp::PointRun` in `plan_chain`, `run_shapes`; t2191
- [x] 3.1 kt-gpu: `FxOp::TapRun`, the generated run text, shapes on the warm worker; t2190, t2192
- [x] 3.2 kt-gpu: the trial by the pass timer, the measured split; t2193
- [x] 3.3 kt-engine: `prewarm_runs`, the player's reach; t2196; preview = export t2197
- [x] 4.1 `kt-cli fuse-cost`; measure bytes and time per frame before/after (release, llvmpipe), the heavy showcase (`preview`, `pause-scan`), CPU at rest; design D-measure
- [x] 4.2 Docs: kt-gpu/kt-fx/kt-plugin-api CLAUDE.md, engine-audit row 10, feature note, What's new
- [x] 5.1 Tests of the specs, `cargo test` of what changed, clippy, live gpufx/render/renderregion/idle
- [x] 5.2 Archive: specs merged
