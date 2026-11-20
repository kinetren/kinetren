# Tasks: picture-check

> DESIGN ONLY until the owner answers Q1–Q2. Test numbers pending (tPENDING-8..10).

## 0. Notes

- [x] 0.1 note F-EXP-009
- [x] 0.2 proposal, design (D1–D5, class walk), deltas `export`, `ui-reach`; `openspec validate picture-check --strict`
- [ ] 0.3 test numbers from the orchestrator; the owner's answers folded in

## 1. Build

- [ ] 1.1 kt-model `lint::check` (flash, gap, missing, bars); tPENDING-9
- [ ] 1.2 kt-engine per-frame statistics on collected frames, the span reducer, `ExportReport.picture`; tPENDING-8
- [ ] 1.3 kt-app: the render window's Picture section, Edit → Check the edit (Diagnostics section), jumps, Mark all; i18n ×5; tPENDING-10

## 2. Close

- [ ] 2.1 What's new; F-EXP-009 verdict; registry T15 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
