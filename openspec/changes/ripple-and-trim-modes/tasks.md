# Tasks: ripple-and-trim-modes

> DESIGN ONLY until the owner answers Q1–Q3 (`proposal.md`). Tests reserved: t3415–t3439.

## 0. Notes first

- [x] 0.1 F-TL-001, F-TL-002, F-TL-008 refreshed with the current code and pointed at this change
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `ui-timeline`, `ui-reach`, `morpher`; `openspec validate ripple-and-trim-modes --strict`
- [ ] 0.3 the owner's answers Q1–Q3 in `docs/user-feedback.md`, folded into the design

## 1. The model (kt-model)

- [ ] 1.1 `RippleScope`, `ops::ripple_plan` (vertical boundary, link groups once, spanning refusal, markers ordered, areas/ranges stretched, morpher blocks, cameras); `Indexed` for per-event questions (builder-cost-guard); t3415, t3416, t3417, t3418
- [ ] 1.2 commands `MoveForeign`, `SetTimelineEnd` (+ `touches_geometry`, `Band::after_batch` rows); t3415
- [ ] 1.3 rippled delete with `restore_handles` first; t3419
- [ ] 1.4 rippled region cut by `Split` + `TrimEnd` (ids kept); t3420; render continuity t3421
- [ ] 1.5 `Command::Slip`, slide and roll batches with clamps from `MediaRef.duration`; t3426, t3428, t3429
- [ ] 1.6 `ops::close_gaps`; t3430
- [ ] 1.7 the reference-layer guard; t3432
- [ ] 1.8 `EditFootprint` set by the edit commands, cleared otherwise and on undo

## 2. The interface (kt-app)

- [ ] 2.1 `controls.rs` rows (post-edit ripple ×3, auto-ripple, close gaps, slip/slide/roll modifiers) in every layout; menus; t3431
- [ ] 2.2 toolbar three-state button, red outline and playhead, the mode hint (rule 19); t3423, t3424
- [ ] 2.3 `plan_move` classes for slip/slide/roll; error-style stops; refusals highlight the obstacle (rule 16); t3422, t3427
- [ ] 2.4 i18n ×5, font subsets; layout_tests, reach_tests, i18n_tests, font_tests green
- [ ] 2.5 live scenario `src/scripts/live/ripple.py` (delete with ripple, region cut, slip to the stop), a screenshot per step; t3425

## 3. Close

- [ ] 3.1 What's new line; F-TL-001/002/008 verdicts → `CAPABILITY: openspec/specs/ui-timeline/`
- [ ] 3.2 idle check with the mode on (live `idle`)
- [ ] 3.3 `src/scripts/finish.sh` (orchestrator, per stage), version in the report
