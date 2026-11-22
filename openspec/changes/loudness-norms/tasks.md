# Tasks: loudness-norms

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Revision 2 (B6). Tests: t3919–t3923, t3966
> (t3967–t3971 spare).

## 0. Notes

- [x] 0.1 F-AUD-005 refreshed; the professional path read (Resolve, Premiere; forums, unverified against manuals)
- [x] 0.2 proposal, design (D1–D6), deltas `audio-render`, `export` (MODIFIED), `ui-reach`, `project-format`; `openspec validate loudness-norms --strict`
- [ ] 0.3 the owner's answer folded in

## 1. Model and format

- [ ] 1.1 `ProjectSettings.loudness`, `Project.program`; format by content; t3966

## 2. Engine

- [ ] 2.1 the program stage after the master sum (gain + true-peak limiter) in the player and the export; t3919, t407
- [ ] 2.2 the program measurement job; per-event matching with short events grouped; t3920
- [ ] 2.3 per-channel accumulators, the report against the project target; t3921, t406

## 3. Interface

- [ ] 3.1 Project settings → Sound, "Match loudness…", the program stage in the meters column, the report table; i18n ×5; t3922, t3923; live `render_norms`

## 4. Close

- [ ] 4.1 What's new; F-AUD-005 refreshed; registry T14 and B6 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
