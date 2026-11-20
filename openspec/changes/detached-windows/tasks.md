# Tasks: detached-windows

> DESIGN ONLY until the owner answers Q1–Q2. Tests: t3901–t3903.

## 0. Notes

- [x] 0.1 note F-UI-012
- [x] 0.2 proposal, design (D1–D5, class walk), delta `ui-reach`; `openspec validate detached-windows --strict`
- [ ] 0.3 test numbers from the orchestrator's block; the owner's answers folded in

## 1. Build (kt-app)

- [ ] 1.1 `Shell::detachable`, the detached host (immediate viewport, own title row, StartDrag/BeginResize with the pointer release), dock by button and by drop; t3901
- [ ] 1.2 the settings store: detached flag, size, position (not on Wayland); the guards t779/t921/t1125 for both hosts; t3902
- [ ] 1.3 the per-surface repaint gate; the `idle` and a playing measurement; t3903
- [ ] 1.4 i18n ×5; the planned live scenario `detached` written with the scenario

## 2. Close

- [ ] 2.1 What's new; F-UI-012 verdict; registry W2 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
