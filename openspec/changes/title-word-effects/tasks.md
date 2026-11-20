# Tasks: title-word-effects

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Tests: t3942–t3946 (t3947 spare).

## 0. Notes

- [x] 0.1 note F-TXT-010
- [x] 0.2 proposal, design (D1–D6), deltas `generators`, `video-fx`, `ui-reach`, `project-format`; `openspec validate title-word-effects --strict`
- [ ] 0.3 the owner's answer folded in

## 1. Build

- [ ] 1.1 kt-text `range_coverage` from the title's layout; t3942
- [ ] 1.2 kt-model `MaskShape::TextRange`, `MaskMix::Isolate`, offsets moved by text edits; format by content; t3943, t3946
- [ ] 1.3 the isolate op in the plan and the GPU passes; t3944
- [ ] 1.4 "Mask this text" in the field, the mask list row, the glyph outline; i18n ×5; t3945, live `word_blur`

## 2. Close

- [ ] 2.1 What's new; F-TXT-010 verdict; registry TD1 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
