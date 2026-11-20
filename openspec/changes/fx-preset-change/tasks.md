# Tasks: fx-preset-change

> DESIGN ONLY until the owner's word and his answer to Q1. Tests: t3595–t3596.

## 0. Notes

- [x] 0.1 note F-UI-010 (`docs/features/ui/editing/change-effect-preset.md`)
- [x] 0.2 proposal, design (D1–D4, class walk), deltas `ui-reach`, `automation`; `openspec validate fx-preset-change --strict`

## 1. Build (kt-app)

- [ ] 1.1 `variants_of(K)` shared by the picker and the slot head; library single-slot presets of K
- [ ] 1.2 the Preset menu at the form head, apply as one `SetFx` keeping curved rows; the reading; i18n ×5; t3595, t3596
- [ ] 1.3 the same head in the node editor's slot view and the morpher patch

## 2. Close

- [ ] 2.1 What's new; F-UI-010 verdict → `CAPABILITY: openspec/specs/ui-reach/`; registry E6 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
