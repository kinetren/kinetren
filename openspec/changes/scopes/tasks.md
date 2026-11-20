# Tasks: scopes

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3400–t3414.

## 0. Notes first

- [x] 0.1 F-CLR-005 refreshed with the current preview path (no readback; the glow precedent)
- [x] 0.2 proposal, design (D1–D7, class walk), spec deltas `scopes`, `ui-reach`; `openspec validate scopes --strict`
- [ ] 0.3 the owner's answers Q1–Q2 recorded in `docs/user-feedback.md` and folded into the design

## 1. The tap (kt-engine, kt-gpu)

- [ ] 1.1 `PresentOptions.scope`, `ScopeTap`, `ScopeSink`; the CPU rule `scope_reduce` in kt-engine
- [ ] 1.2 kt-gpu `scope_down.wgsl` (mean + min + max, two integer targets), the two-deep map ring polled in `present` and on a replayed kept composite; ledger rows; t3400
- [ ] 1.3 `Player::measure_paused` on the offline route, memo by plan key, a `Visible` scheduler job cancelled by play; t3401, t3402

## 2. The instruments (kt-app)

- [ ] 2.1 `scopes.rs` on its own thread: waveform, parade, vectorscope, histogram, false colour zones; one reused image buffer, `wake::now()`; t3403, t3404, t3405
- [ ] 2.2 the window (shell, `WINDOWS` row with reason), View and preview right-click entries, `UiState.scopes`; i18n ×5, font subsets; t3411
- [ ] 2.3 `Work::ScopeTap`, `Work::ScopeCount`; the `idle` live scene step; t3410
- [ ] 2.4 live scenario `src/scripts/live/scopes.py` with a screenshot per step; t3412
- [ ] 2.5 layout_tests: the head does not jump when instruments change (class t76/t86); reach_tests, font_tests, i18n_tests green

## 3. Close

- [ ] 3.1 What's new line (`docs/guide/whats-new.md`); F-CLR-005 verdict → `CAPABILITY: openspec/specs/scopes/`
- [ ] 3.2 idle measurement on a heavy project (pause, mouse over the scope, window open) in the report
- [ ] 3.3 `src/scripts/finish.sh` (run by the orchestrator per stage), version in the report
