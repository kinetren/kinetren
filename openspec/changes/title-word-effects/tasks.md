# Tasks: title-word-effects

> DESIGN ONLY until the orchestrator's go; revision 2 (B8 decided). Tests: t3942–t3946, t4246.

## 0. Notes

- [x] 0.1 note F-TXT-010
- [x] 0.2 proposal, design (D1–D6), deltas `generators`, `video-fx`, `ui-reach`, `project-format`; `openspec validate title-word-effects --strict`
- [x] 0.3 the owner's answer (B8) folded in: many spans in one mask, one rasterization per title (D7)

## 1. Build

- [ ] 1.1 kt-text `range_coverage` from the title's layout; the id map of mask groups; t3942, t4246
- [ ] 1.2 kt-model `MaskShape::TextRange`, `MaskMix::Isolate`, offsets moved by text edits; format by content; t3943, t3946
- [ ] 1.3 the isolate op in the plan and the GPU passes; t3944
- [ ] 1.4 "Mask this text" in the field, the mask list row, the glyph outline; i18n ×5; t3945, live `word_blur`

## 2. Close

- [ ] 2.1 What's new; F-TXT-010 verdict; registry TD1 moved
- [ ] 2.2 `src/scripts/finish.sh` (orchestrator), version in the report
