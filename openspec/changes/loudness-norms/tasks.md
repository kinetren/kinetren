# Tasks: loudness-norms

> Revision 3. Tests: t3919–t3923, t3966, t4200–t4203; from
> the branch t3106, t3107.

## 0. Notes

- [x] 0.1 F-AUD-005 refreshed; the professional path read (Resolve, Premiere; forums, unverified against manuals)
- [x] 0.2 proposal, design (D1–D8), deltas `audio-render`, `export` (MODIFIED + ADDED), `ui-reach`, `project-format`; `openspec validate loudness-norms --strict`

## 1. The unmerged half

- [x] 1.1 `git merge --no-ff render-loudness-norms` (53fe3a33, 6eec28ce; the orchestrator confirmed it 2026-11-24). It
      brings t3106, t3107 and the live scenario `render_loudness`, its archived change `2026-11-18-render-loudness-norms`
      and its text in `openspec/specs/export/`. Six files conflict at `0c2a4065` (`git merge-tree` run there); the others
      merge by themselves:
  - `docs/features/README.md`, the folder table: keep OUR rows (theirs are of 2026-11-18); only `export/` moves to
    `10 / 3`. In the list of notes keep every row of ours and add theirs under the new number (1.2), after F-EXP-008.
  - `docs/guide/whats-new.md`: both sides added lines at the top of `## kt26.11`; keep all of ours and put theirs first.
  - `docs/ktprompts/v26.10-index.md`: T12 — OURS (already done). T14 — neither: state "partial", naming t3106/t3107 and
    `render_loudness` for the table and channels, and this change for the program loudness and the matching.
  - `src/apps/kt-app/assets/NotoSansSC-kt.otf` and `SUBSET-MANIFEST.txt`: take OURS, then
    `python3 src/scripts/subset-fonts.py` over the merged catalogs (ours has 944 code points, theirs 939; the
    result is a new set). t125/t1283 and `font_tests.rs` say whether it is right.
  - `src/apps/kt-app/src/layout_tests.rs`: both sides appended at the end; keep ours whole and put `t3107` after it.
- [x] 1.2 Their note `docs/features/export/pipeline/render-loudness-report.md` is F-EXP-009, taken by `picture-check.md`:
      it becomes **F-EXP-010** in the note, the README row, t3107's comment, `render_loudness.py`, the archived change and
      `docs/ktprompts/v26.10-index.md`; `python3 src/scripts/features-index.py --check`
- [x] 1.3 What the merge cannot see: `Loudness` gains `channels` and the true-peak gate becomes per channel. Today it is
      built in one place (`loudness.rs:270`); t2174 (`kernel_specialization.rs`) holds the bits of LUFS, LRA and dBTP and
      stays green as it is — a red there is a cause to find, not numbers to print again

## 2. Model and format

- [x] 2.1 `Project.loudness` (a field of the settings would touch 216 literals; the document's either way), `Project.program`, `SetLoudness`, `SetProgram`, the `sound` hash; format by content; t3966

## 3. Engine

- [x] 3.1 the kernel `kt_fx::audio::program`; t4201
- [x] 3.2 the stage at the player's output and in `render_raw_audio_range` (every caller of design D3's table); t3919, t4200, t4202, t407
- [x] 3.3 the jobs: program (two passes), events with short ones grouped; `AnalysisKind` and t774's lock by name; t3920, t4203
- [x] 3.4 the report against the project's target, the spans; t3921, t406

## 4. Interface

- [x] 4.1 Project settings → Sound, "Match loudness…", the stage in the meters column, the report; kt-cli export's lines; i18n ×5; t3922, t3923; live `render_norms`
- [x] 4.2 the limiter effect's hint (sample peaks)
- [ ] 4.3 the load check: playing the heavy showcase with the stage on, paused with it on (nothing)

## 5. Close

- [x] 5.1 What's new; F-AUD-005's verdict; registry T14 and B6 moved; OTIO row "planned" → the change's date
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report
