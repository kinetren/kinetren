# Tasks: auto-ducking-by-role

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Revision 2 (B5). Tests: t3911–t3918.

## 0. Notes

- [x] 0.1 note F-AUD-007 (the owner's whole item quoted)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `audio-fx`, `ui-reach`, `project-format`; planned OTIO row; `openspec validate auto-ducking-by-role --strict`
- [ ] 0.3 the owner's answer folded in

## 1. Model and format

- [ ] 1.1 `AudioRole`, `SidechainKey::Set`, `Ducking` rules, title spans indexed; cycle refusal; t3914
- [ ] 1.2 format by content; R.65 projects unchanged; t3918

## 2. Engine

- [ ] 2.1 the key mix of several tracks and roles; the content rule's closed-form mapping; t3911
- [ ] 2.2 the title envelope (raise, mute, only-in-titles); t3912
- [ ] 2.3 the hand level summed after the automatic gain; t3913
- [ ] 2.4 the band analysis cached by content

## 3. Interface

- [ ] 3.1 role menus, the section, Details dimmed, user_store defaults; i18n ×5; t3915
- [ ] 3.2 the band mesh, Convert to volume curve; t3916, t3917; live `ducking_roles`
- [ ] 3.3 idle measured on a heavy project

## 4. Close

- [ ] 4.1 What's new; F-AUD-007 verdict; registry N3 and B5 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
