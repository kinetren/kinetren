# Tasks: frame-format-variants

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3485–t3504.

## 0. Notes first

- [x] 0.1 note F-PRJ-006 (`docs/features/project/structure/frame-format-variants.md`)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `video-plan`, `export`, `project-format`, `ui-reach`; `openspec validate frame-format-variants --strict`
- [x] 0.3 planned row in "What OTIO export loses"
- [ ] 0.4 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The model (kt-model)

- [ ] 1.1 `FrameFormat`, `FormatOverride`, track masks per format, commands, validate; t3489
- [ ] 1.2 `effective_clips_for(target)` (overrides, presence, rule) in the one door

## 2. The engine (kt-engine)

- [ ] 2.1 `RenderTarget` into the builds; `LeadCtx.raster`; the audio build skipped on a target-only change; t3485, t3486
- [ ] 2.2 keys: the disk tier's durable key adds the target raster; t3487
- [ ] 2.3 export of a format; queue of formats; kt-cli `--format`; t3488
- [ ] 2.4 the class-walk readers (titles, camera world scale, masks, subtitles) take the target raster

## 3. The interface (kt-app)

- [ ] 3.1 preview head chooser, the settings section Formats with presets
- [ ] 3.2 gizmos and Geometry fields write the shown format's override; override mark; reset/hide items; t3491
- [ ] 3.3 safe-area overlays (presets per Q2)
- [ ] 3.4 render window Formats list; i18n ×5; live scenario `formats.py`; t3492
- [ ] 3.5 kt-otio metadata; t3490

## 4. Close

- [ ] 4.1 What's new; F-PRJ-006 verdict → `CAPABILITY: openspec/specs/video-plan/`
- [ ] 4.2 idle check: a format shown, pause, mouse over the preview
- [ ] 4.3 `src/scripts/finish.sh` (orchestrator), version in the report
