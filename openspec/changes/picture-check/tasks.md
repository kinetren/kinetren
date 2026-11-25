# Tasks: picture-check

> Waits for Q72, Q73 (markers and the flashing row only; 1.1 and 1.2 without `flash` need neither). `loudness-norms` is merged. Tests: t3908–t3910.

## 0. Notes

- [x] 0.1 note F-EXP-009
- [x] 0.2 proposal, design (D1–D5, class walk), deltas `export`, `ui-reach`; `openspec validate picture-check --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Build

- [ ] 1.1 kt-model `lint::check` (flash, gap, missing, bars); t3909
- [ ] 1.2 kt-engine per-frame statistics on collected frames, the span reducer, `ExportReport.picture`; t3908
- [ ] 1.3 kt-app: the render window's Picture section, Edit → Check the edit (Diagnostics section), jumps, Mark all; i18n ×5; t3910

## 2. Close

- [ ] 2.1 What's new; F-EXP-009 verdict; registry T15 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
