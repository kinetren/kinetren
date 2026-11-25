# Tasks: openfx-host

> IMPLEMENTATION-READY (revision 3, 2026-11-25, orch `fc60a5d9`: design D11–D15). After `wasm-plugin-host` (the trial's door, the faulted kind, the marks). Tests: t3974–t3979, t4153, t4154. Areas: new `kt-plugin-ofx`, GPU (`context.rs`, a foreign op in `engine.rs`), the plan's planner (`kt-fx/plan.rs` one op), the two mains, SHELL, FX-UI. Not beside `export-into-surface` 3
> (`context.rs`, hal) or `track-stream-stage` 2 (`engine.rs`). The network is needed once: the OpenFX headers and the open sets (openfx-misc built by the fixture script, cached). Commit per section; target-prune after each.

## 0. Notes

- [x] 0.1 F-EXT-001 refreshed
- [x] 0.2 proposal (the owner's whole item and this design's reading), design (D1–D9 with the answers to items 2–4), deltas `plugins`, `ui-reach`; `openspec validate openfx-host --strict`
- [x] 0.3 the owner's answers folded in (D4: D10 shared frames; Q1 a, Q2 a)

## 1. The child (RAM path with the counted copy)

- [ ] 1.1 vendored headers; `--ofx-host`, the protocol, the suites, the scan and its cache; a test plugin of ours in C (filter, generator, transition, stateful, crashing, hanging, temporal) built by the test; t3976
- [ ] 1.2 supervision, budgets, the faulted kind; t3975
- [ ] 1.3 `fixtures/ofx.sh` builds openfx-misc at a pinned commit (opt-in, loud: `KT_EXPECT_OFX=1`); commit

## 2. The plan

- [ ] 2.1 `PlanOp::Foreign`, `ForeignFx`, the lane of its own, the keys, `durable`; t3974
- [ ] 2.2 temporal reads by the declared frames; t4153
- [ ] 2.3 parameters, curves, parametric, buttons; t4154; `cargo test -p kt-engine -p kt-fx -p kt-plugin-ofx`; commit (useful alone: merge)

## 3. Frames without a copy

- [ ] 3.1 `kt_gpu::context::open` (D11) behind every device; the machine facts; every GPU test green on it; commit alone
- [ ] 3.2 host-memory buffers; t3979
- [ ] 3.3 exported images, the child's GL (then CUDA, OpenCL) imports, the UUID check; t3978 (opt-in where the software stack cannot; the C row's text to the orchestrator); commit

## 4. Interface

- [ ] 4.1 picker marks, hints, notices, "does not load" rows, Library rows; i18n ×5; t3977, live `ofx_plugin` (OUR test plugin; a screenshot per step)
- [ ] 4.2 the idle check: the child asleep with the effects window open on the heavy showcase

## 5. Close

- [ ] 5.1 the answers to O2–O4 in the guide's page; What's new; F-EXT-001 verdict; registry O1–O4; kt-gpu `CLAUDE.md` (where hal is touched); commit
