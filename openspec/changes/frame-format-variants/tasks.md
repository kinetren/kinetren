# Tasks: frame-format-variants

> Decided 2026-11-20 (A17); revision 2 implementation-ready at `229c694b` (2026-11-24). Code starts on the orchestrator's go,
> after `shared-events` and `preview-canvas-and-gizmos`. Tests: t3485–t3492, t4235–t4238.

## 0. Notes

- [x] 0.1 F-PRJ-006; proposal, design (D0–D8), deltas `video-plan`, `export`, `project-format`, `ui-reach`, `generators`;
      `openspec validate frame-format-variants --strict`
- [x] 0.2 his answers Q26–Q27 (kkt-26.11.20 lines 26–27, A17), read against ★15
- [x] 0.3 planned row in "What OTIO export loses"

## 1. The model (kt-model)

- [ ] 1.1 `FrameFormat`, `Framing`, `format_masks`, the commands (with `target_rows`, `touches_render_plan`, `Band::after_batch`,
      telemetry's label), slices and blocks carry the framing; validate; t3489
- [ ] 1.2 `Project::in_format` (order of D2, hidden events, curves shifted and scaled); t3485, t3486, t4235
- [ ] 1.3 the height-sized rows of generator kinds named in their passports, scaled by the short side; t4236

## 2. Titles (kt-text, kt-media)

- [ ] 2.1 `kt_text::wrap_lines`, `TitleWrap` from kt-media; t4237

## 3. The engine and the render

- [ ] 3.1 `Project.shown` in the plan key; t3487
- [ ] 3.2 kt-cli `--format`; the render queue and its window card; t3488, t4238

## 4. The interface (kt-app)

- [ ] 4.1 the chooser, the settings section, `push_project_to_player`; the poster and the picker's scene take the same project
- [ ] 4.2 the geometry frame and the form's Framing section, the mark, the menu items; t3491
- [ ] 4.3 safe areas; i18n ×5, font subsets; live `formats` (t3492) with a screenshot per step
- [ ] 4.4 kt-otio metadata; t3490

## 5. Close

- [ ] 5.1 What's new ("one edit in several frame formats: a vertical and a square beside 16:9, each with its own framing, rendered
      in one press"); F-PRJ-006 verdict; P1 ★15 and A17 moved
- [ ] 5.2 idle with a format shown on the heavy showcase; `finish.sh` (orchestrator)
