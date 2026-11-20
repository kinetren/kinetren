# Tasks: fx-preset-change

> DESIGN ONLY until the orchestrator's go. Revision 2: the owner's A4 decided it. Tests: t3595–t3596, t3932–t3934.

## 0. Notes

- [x] 0.1 note F-UI-010 (`docs/features/ui/editing/change-effect-preset.md`), the owner's decision folded in
- [x] 0.2 proposal, design (D1–D6, class walk), deltas `ui-reach`, `automation`; `openspec validate fx-preset-change --strict`

## 1. Build

- [ ] 1.1 `variants_of(K)` shared by the picker and the slot head; library single-slot presets of K
- [ ] 1.2 the Preset menu at the form head, replacing values and curves in one step; the reading; i18n ×5; t3595, t3596
- [ ] 1.3 kt-model `ops::insert_chain_block_at`; the chain preset and paste inserted after the selection; t3932, t3934
- [ ] 1.4 the selection set, Ctrl+click, the selection frame, Delete of a selection, the group drag; t3933
- [ ] 1.5 the same head and selection in the track and morpher graphs; on v2 (`fx-graph-dag`) the insertion on the arm

## 2. Close

- [ ] 2.1 What's new; F-UI-010 verdict → `CAPABILITY: openspec/specs/ui-reach/`; registry E6 and kkt-26.11.20 A4 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
