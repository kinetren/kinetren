# Tasks: auto-ducking-by-role

> DESIGN ONLY until the owner answers Q1–Q2. Tests: t3911–t3918.

## 0. Notes

- [x] 0.1 note F-AUD-007
- [x] 0.2 proposal, design (D1–D7, class walk), deltas `audio-fx`, `ui-reach`, `project-format`; planned OTIO-loses row; `openspec validate auto-ducking-by-role --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Model and format

- [ ] 1.1 `AudioRole`, `Track.role`, `ClipEvent.role`, effective role; `SidechainKey::Roles`; cycle validation; t3912
- [ ] 1.2 the `.kt` version by content, the OTIO envelope; t3918; the OTIO-loses row moves from planned

## 2. Engine

- [ ] 2.1 role key mixes per window from post-chain event outputs, render order; t3911
- [ ] 2.2 the form's closed-form mapping onto the compressor; t3913
- [ ] 2.3 the reduction analysis (the compressor's detector and gain code), cached by content hash; t3914

## 3. Interface

- [ ] 3.1 role menus (track header, event, track window), waveform colours; i18n ×5
- [ ] 3.2 the Auto-ducking section (Details dimmed, custom mark, user_store defaults); t3915
- [ ] 3.3 the band mesh, Convert to volume curve; t3916, t3917; live scenario `ducking_roles`
- [ ] 3.4 idle measured on a project with heavy effects (pause, hover, the track window open)

## 4. Close

- [ ] 4.1 What's new; F-AUD-007 verdict; registry N3 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
