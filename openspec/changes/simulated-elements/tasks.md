# Tasks: simulated-elements

> DESIGN ONLY until the owner answers Q1–Q2 and the orchestrator's go. The engine contract (design D10) is anchored
> to orch `d613b228` and reviewed with spec-engine. Revision 2 (D3): the system, then three
> applications (Q1 a). Tests: t3968–t3973, t3983–t3989 (t3990 reserved). Depends on `motion-driven-params` (the outer
> map's channels); the mask application on `point-tracker`.

## 0. Notes

- [x] 0.1 F-FX-005 refreshed (the system); F-FX-006 points to it for particles
- [x] 0.2 proposal (the owner's whole items and this design's reading), design (D1–D10, class walk D9), deltas `simulation` (new), `video-plan`, `project-format`, `ui-reach`; planned OTIO row renamed; `openspec validate simulated-elements --strict`
- [ ] 0.3 the owner's answers folded in

## 1. The system (kt-sim, kt-plugin-api)

- [ ] 1.1 `ElementSet`, `Element`, `Pose`, operators; the `sim` family (`SimKernel`, registry, versions); t3984
- [ ] 1.2 the integrator: the outer clock per instance path, fixed step, frozen math, interpolation, memory checkpoints; t3968, t3970
- [ ] 1.3 drivers (outer channels, home acceleration, links, sound analysis, tracker, other sets) and the dependency order; t3985

## 2. In the plan (kt-engine, kt-gpu) — design D10, D11

- [ ] 2.1 `frame_plan` (`kt-engine/src/video.rs:2360`) asks the sets for poses; the outer clock and the instance
      path go down the nested build beside `NestSrc.inner_t` (design D11); simulated mask vertices replace the
      evaluated ones in `PlanLayer.masks` (`video_plan.rs:118`); `plan_key` (`kt-gpu/src/engine.rs:4330`) and
      `put_plan` (`render_cache.rs:715`) hash the poses (t2981 kept green); zero cost without sets; t3969, t3971,
      t3987
- [ ] 2.2 the incomplete-frame report beside `note_stream_short` (`video.rs:2399`) and the catch-up line; the advance
      off the compose thread beside `stream_advance_ahead` (`player.rs:3609`); the export's forward integration
      beside `stream_preroll` (`export.rs:510`); checkpoints as a new ledger tenant (`kt-time/src/mem.rs`); the
      ceiling; t3986
- [ ] 2.3 the instanced-sprite pass in kt-gpu (a `Compiled` pipe with its `Slot`, timed, warmed with the project);
      a simulated title's letters through it from the source's glyph atlas (design D11; `GeneratorSource`,
      `kt-media/src/generator.rs:351`, rasterizes the atlas once); a title with no set keeps its path; t4107
- [ ] 2.4 the probe plan keys a set without stepping (`Sim::key_only(SimKey, t)`), so `show_kept` finds kept moments
      without the simulation; the probe key equals the full plan's
- [ ] 2.5 the Emitter: its source draws the sprite (atlas) once, the plan carries the particles' poses, the same
      instanced pass draws them; warmed by `prewarm::effect_ids`

## 3. Model and format

- [ ] 3.1 `ElementSource`, `SimRule` rows, element addresses for links; format by content; t3973

## 4. Applications (Q1 a)

- [ ] 4.1 letters: title clusters/words/lines as a producer, poses at rasterization; spring, pendulum, rope, fall solvers in `plugins/sim-basic`
- [ ] 4.2 mask vertices with a tracker home
- [ ] 4.3 the Emitter generator and the particles solver

## 5. Interface

- [ ] 5.1 the "Simulation" section in the title form, the mask panel and the emitter's form; the homes overlay; i18n ×5; t3972, t3988, t3989; planned live scenario `simulated_elements`
- [ ] 5.2 the idle check on the heavy showcase with a simulated title and an emitter

## 6. Close

- [ ] 6.1 What's new; F-FX-005 and F-FX-006 verdicts; registry F2 and D3 moved
- [ ] 6.2 `src/scripts/finish.sh` (orchestrator), version in the report
