# Tasks: motion-driven-params

> IMPLEMENTATION-READY (revision 3, 2026-11-25, orch `fc60a5d9`; the owner's B7 decided). Tests: t3947–t3951, t3967, t4146, t4147. Areas: MODEL (`project.rs` `CurveSource`, `autoplan.rs`, `links.rs`, new `outer.rs`, **v+**), `video.rs`, `video_stream.rs`,
> AUDIO (`audio.rs`), FX-UI (`automation.rs`). After `shared-events` (MODEL, the builders' keys); not beside `track-stream-stage` 1.2–2.2 (`layer_src`, `PlanCtx`). Read: design D1–D4; kt-model and kt-engine `CLAUDE.md`. Commit per section; target-prune after each.

## 0. Notes

- [x] 0.1 note F-AUT-014 (revised: the edit's speed)
- [x] 0.2 proposal, design (D1–D8, class walk D7), deltas `automation`, `compositions`, `ui-reach`, `project-format`; `--strict`

## 1. Model

- [ ] 1.1 `CurveSource::Outer`, its checks and refusals, `AutoPlan` with `OuterNow`, `apply_fx_auto`; GOLDEN and t191 untouched
- [ ] 1.2 `outer.rs` (D2), `ClipWarp::rate_at`; t3947, t3949
- [ ] 1.3 format and OTIO by content; t3967; commit

## 2. The picture

- [ ] 2.1 `PlanCtx.outer`, `layer_src`, `ClipMaskPlan::ops`, `ChainMemo`, `LayerSrc.outer` in `moment_key`; t3948, t3950; t442, t443, t2004 green
- [ ] 2.2 `OuterPath::first`, the stream stage's rows, front, `plan_cell`, `ident`; t4146
- [ ] 2.3 the decode step over the channels' range; `kt-cli probe`; `cargo test -p kt-engine`; commit

## 3. The sound

- [ ] 3.1 the path in `build_reel`, the nest key, the sampled keys, `fp`; t4147; GOLDEN_AUDIO, t2003–t2005 green; commit

## 4. Interface

- [ ] 4.1 "Driven by…" → "The speed it is played at" (D5); i18n ×5; reach, layout, font tests; t3951, live `motion_link` (OUR fixture: a reel placed twice, one braking to a stop; a screenshot per step)
- [ ] 4.2 the idle check with a driven effect on the heavy showcase (pause, mouse over a static place, the effects window open)

## 5. Close

- [ ] 5.1 What's new ("an effect inside a composition can follow the speed the composition is played at"); F-AUT-014 verdict; registry TD2 and B7; kt-model `CLAUDE.md` ("a curve source that depends on who plays the event is asked at the plan, never made keys for the picture");
      the report's "Найдено, не исправлено"; commit
