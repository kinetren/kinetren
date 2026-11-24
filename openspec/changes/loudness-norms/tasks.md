# Tasks: loudness-norms

> DESIGN ONLY until the orchestrator's go; no open question. Revision 3. Tests: t3919–t3923, t3966, t4200–t4203; from
> the branch t3106, t3107.

## 0. Notes

- [x] 0.1 F-AUD-005 refreshed; the professional path read (Resolve, Premiere; forums, unverified against manuals)
- [x] 0.2 proposal, design (D1–D8), deltas `audio-render`, `export` (MODIFIED + ADDED), `ui-reach`, `project-format`; `openspec validate loudness-norms --strict`

## 1. The unmerged half

- [ ] 1.1 Merge the branch `render-loudness-norms` (conflicts at `286c406c`: features README, What's new, the v26.10 registry, the Chinese subset and its manifest (rebuild), `layout_tests.rs`); its note renumbered F-EXP-009 → F-EXP-010; t3106, t3107, live `render_loudness` green

## 2. Model and format

- [ ] 2.1 `ProjectSettings.loudness`, `Project.program`, `SetLoudness`, `SetProgram`, the `sound` hash; format by content; t3966

## 3. Engine

- [ ] 3.1 the kernel `kt_fx::audio::program`; t4201
- [ ] 3.2 the stage at the player's output and in `render_raw_audio_range` (every caller of design D3's table); t3919, t4200, t4202, t407
- [ ] 3.3 the jobs: program (two passes), events with short ones grouped; `AnalysisKind` and t774's lock by name; t3920, t4203
- [ ] 3.4 the report against the project's target, the spans; t3921, t406

## 4. Interface

- [ ] 4.1 Project settings → Sound, "Match loudness…", the stage in the meters column, the report; kt-cli render's lines; i18n ×5; t3922, t3923; live `render_norms`
- [ ] 4.2 the limiter effect's hint (sample peaks)
- [ ] 4.3 the load check: playing the heavy showcase with the stage on, paused with it on (nothing)

## 5. Close

- [ ] 5.1 What's new; F-AUD-005's verdict; registry T14 and B6 moved; OTIO row "planned" → the change's date
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report
