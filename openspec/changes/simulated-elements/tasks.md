# Tasks: simulated-elements

> DESIGN ONLY until the orchestrator's go; no open question. Engine contract (D10, D12) reviewed with spec-engine at
> `d613b228`, anchors re-checked at `5986f08e` (D13–D18). Tests: t3968–t3973, t3983–t3992, t4107. Depends on
> `motion-driven-params` (the outer map's channels, D13 `Outer`); the tracker home of a mask works when `point-tracker`
> is in (a link to its samples), and is not needed to build this.

## 0. Notes

- [x] 0.1 F-FX-005 refreshed (the system); F-FX-006 points to it for particles
- [x] 0.2 proposal (the owner's whole items and this design's reading), design (D1–D10, class walk D9), deltas `simulation` (new), `video-plan`, `project-format`, `ui-reach`; planned OTIO row renamed; `openspec validate simulated-elements --strict`
- [x] 0.3 Q1, Q2 decided by the owner's words; D13–D18

## 1. The system (kt-sim, kt-plugin-api)

- [ ] 1.1 `ElementSet`, `Element`, `Pose`, operators; the `sim` family (`SimKernel`, registry, versions); t3984
- [ ] 1.2 the integrator: the outer clock per instance path, fixed step, frozen math, interpolation, memory checkpoints; t3968, t3970
- [ ] 1.3 drivers (outer channels, home acceleration, links, sound analysis, tracker, other sets) and the dependency order; t3985

- [ ] 1.4 the reference frame and exact homes (D15); t3991

## 2. In the plan (kt-engine, kt-gpu) — design D10, D12

- [ ] 2.1 plan-side sets: `layer_src` (`video.rs:2497`) asks `Sim::state_at` and writes simulated numbers into
      `LayerSrc.masks` (`video_plan.rs:118`) and the sub-plan transforms; the `SimKey` joins the per-event memo key
      (`incremental.rs`, t2004); zero cost without sets; t3969, t3971, t3987
- [ ] 2.2 source-side producers draw only what does not move (design D12.1): the title's glyph atlas (one sprite per
      cluster, `GeneratorSource`, `kt-media/src/generator.rs:169` `render_at`), the emitter's sprite; the poses of
      letters and particles are a plan-side set in `layer_src` like 2.1; three masks per cluster, two passes (D17); no salt from the `SimKey`, no drivers in
      `Retune::Gen` (a retune drops the source's whole cache, `worker.rs:363–380`); a title or emitter with no set
      keeps salt 0 and its bytes; t4107
- [ ] 2.3 catch-up: the player's `sim` thread (class `near`, joined through `kt_time::shutdown`); `layer_src` calls
      `state_at` with a small budget and on `Behind` notes `note_sim_short` beside `note_stream_short`
      (`video.rs:2399`); the worker never steps; `render_ahead::Waits` on `sim::walked`; `Sim::preroll` in both export
      bodies; t3986
- [ ] 2.4 the key `SimKey(t)` over the drivers' prefix [origin, t] (design D12.2, the table of D16); sound not analysed
      answers `Behind` (t3992); the plan and the probe hash
      `(SimKey(t), t)`; `Sim::key_only` never steps, so `show_kept` finds kept moments; probe key == full key; the
      per-event memo key takes the static part only (t2004); t3987
- [ ] 2.5 the instanced-sprite op and pass in kt-gpu (design D12.3): a `Compiled` pipe with its `Slot`, timed, cut by
      the arbiter, warmed by `prewarm::effect_ids`; `plan_key` (`engine.rs:4330`) and `put_plan`
      (`render_cache.rs:715`) hash the op (t2981 green)

## 3. Model and format

- [ ] 3.1 `ElementSet`, `SetOp`, `Driver`, `SetSets`, `ParamOwner::Set`, copy/paste/delete rules (D13); format by content; t3973
- [ ] 3.2 `SimSpan` in `Split`, `FrameHold`, `TrimStart` (D14); t3990

## 4. Applications (stage A)

- [ ] 4.1 letters: title clusters/words/lines as a producer, poses at rasterization; spring, pendulum, rope, fall solvers in `plugins/sim-basic`
- [ ] 4.2 mask vertices with a tracker home
- [ ] 4.3 the Emitter generator and the particles solver

## 5. Interface

- [ ] 5.1 the "Simulation" section in the title form, the mask panel and the emitter's form; the homes overlay; i18n ×5; t3972, t3988, t3989; planned live scenario `simulated_elements`
- [ ] 5.2 the idle check on the heavy showcase with a simulated title and an emitter

## 6. Close

- [ ] 6.0 stage B rows (D18) entered in the build queue, one change each
- [ ] 6.1 What's new; F-FX-005 and F-FX-006 verdicts; registry F2 and D3 moved
- [ ] 6.2 `src/scripts/finish.sh` (orchestrator), version in the report
