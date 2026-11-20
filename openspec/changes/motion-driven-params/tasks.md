# Tasks: motion-driven-params

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Tests: t3947–t3951 (t3952 spare).

## 0. Notes

- [x] 0.1 note F-AUT-014
- [x] 0.2 proposal, design (D1–D7), deltas `automation`, `ui-reach`, `project-format`; `openspec validate motion-driven-params --strict`
- [ ] 0.3 the owner's answer folded in

## 1. Model

- [ ] 1.1 speed owners (event, instance, morpher), the channel addresses, the closed-form `accel`/`change`; t3947
- [ ] 1.2 format by content; t3951

## 2. Engine

- [ ] 2.1 the speed channels resolved in `morph_pass`; seek equals continuous; t3948
- [ ] 2.2 the transport channels as frame inputs, offline normal values, `transport_bound` keys, no disk; t3949

## 3. Interface

- [ ] 3.1 "Follow speed change of…", the pick, the preview-only mark; i18n ×5; t3950, live `motion_link`

## 4. Close

- [ ] 4.1 What's new; F-AUT-014 verdict; registry TD2 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
