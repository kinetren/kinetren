# Tasks: text-letter-physics

> DESIGN ONLY until the owner answers Q1–Q2 and the orchestrator's go. Tests: t3968–t3973. Depends on
> `motion-driven-params` (the outer speed).

## 0. Notes

- [x] 0.1 F-FX-005 refreshed
- [x] 0.2 proposal (the owner's whole item and this design's reading of it), design (D1–D7), deltas `generators`, `ui-reach`, `project-format`; planned OTIO row; `openspec validate text-letter-physics --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Model and format

- [ ] 1.1 the animator's rows; format by content; t3973

## 2. Simulation

- [ ] 2.1 homes from the layout, the drive from the outer speed, the integrator and behaviours; t3968, t3971
- [ ] 2.2 checkpoints in memory and the persistent cache; byte-equal with and without; t3969, t3970

## 3. Interface

- [ ] 3.1 the preset, rows, spring lines; i18n ×5; t3972, live `letter_physics`

## 4. Close

- [ ] 4.1 What's new; F-FX-005 verdict; registry F2 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
