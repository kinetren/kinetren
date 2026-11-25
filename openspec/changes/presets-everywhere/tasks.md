# Tasks: presets-everywhere

> Waits for Q68 (the model of "take"), Q69 (content only); re-read 2026-11-25. Tests: t3904–t3907. After `fx-preset-change`.

## 0. Notes

- [x] 0.1 note F-AUT-013
- [x] 0.2 proposal, design (D1–D5, class walk), deltas `automation`, `ui-reach`; `openspec validate presets-everywhere --strict`
- [ ] 0.3 the owner's answers folded in

## 1. The model (kt-model)

- [ ] 1.1 blocks for transitions, generator values, masks, cameras, patches, geometry; paste by each domain; t3904
- [ ] 1.2 `take_from` with a checklist, stretch/keep, skip-and-count; t3906

## 2. The interface (kt-app)

- [ ] 2.1 `PresetStore` per kind (the chain file unchanged), factory presets per Q2; t3905
- [ ] 2.2 `entity_head` on every entity form + the guard; the pick mode; the checklist window (registry row with reason); i18n ×5; t3907
- [ ] 2.3 the planned live scenario `take_from` written with the scenario

## 3. Close

- [ ] 3.1 What's new; F-AUT-013 verdict; registry A2/A3 moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report
