# Tasks: loudness-norms

> DESIGN ONLY until the owner answers Q1–Q2. Tests: t3919–t3923.

## 0. Notes

- [x] 0.1 F-AUD-005 refreshed (since 2026-11-20)
- [x] 0.2 proposal, design (D1–D6, class walk), deltas `export`, `ui-reach`; `openspec validate loudness-norms --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Measurement

- [ ] 1.1 kt-fx: per-channel energies and true peak in the accumulator, mixed bytes unchanged; t3920
- [ ] 1.2 kt-engine: the per-burst record, the reducer, `ExportReport.sound`; t3919
- [ ] 1.3 the norm table, `loudness_norm` in the text form and presets; t3921

## 2. Interface

- [ ] 2.1 the Sound table, states, hovers, the offset line, the norm box; i18n ×5; t3922, t3923
- [ ] 2.2 the shared findings list with `picture-check` (whichever lands second joins the first); live scenario `render_norms`

## 3. Close

- [ ] 3.1 What's new; F-AUD-005 refreshed again; registry T14 moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report
