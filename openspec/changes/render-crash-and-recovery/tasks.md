# Tasks

Test numbers t1070–t1084 are reserved by this change. Every stage ends with a commit (English
message, first line "render-crash-and-recovery <stage>: …", no attribution lines) and
`src/scripts/target-prune.py`.

## 0. Records before code

- [x] 0.1 Proposal, design (with the supervisor alternatives), delta specs and tasks written; `openspec validate render-crash-and-recovery --strict` green; `src/scripts/specs-user-path.py --check` accepts the PATH lines
- [x] 0.2 F-HLP-014 (`docs/features/help/errors/crash-and-bug-report-flow.md`) gains the trick "the reporter must already be alive when the program dies" and the link to this change; `python3 src/scripts/features-index.py --check` green
- [x] 0.3 Commit stage 0

## 1. The panic (export)

- [ ] 1.1 `kt_engine::export::video_frames_in` used by `export_mp4_with_backends`'s total; t1070 on off-grid / on-grid / empty / sub-frame regions (red on the floored total)
- [ ] 1.2 `render_dialog.rs`: clamped fraction, `try_from_secs_f64` for remaining and the lengths; `ffmpeg_setup::eta` guarded the same way; t1071 (red on the old formula: the panic)
- [ ] 1.3 Commit stage 1

## 2. The log (journal)

- [ ] 2.1 Local time with offset (`localtime_r` / `GetTimeZoneInformation`), `stamp_with(t, offset)` pure; t1079
- [ ] 2.2 `start_run` (fan of four previous runs, header), size threshold shifts the fan, `last_run_tail`; t1075; t354 stays green
- [ ] 2.3 The `shown:` mark: `take_crash_report` reports whether the window showed it; the start announces only an unshown report; t1078
- [ ] 2.4 Commit stage 2

## 3. Recovery

- [ ] 3.1 `save()` skips a sandbox; `plan_start` drops a sandbox `last_project`; t1076
- [ ] 3.2 `restore_recovery` builds a new editor (`open_loaded` / `new_with_project`) through `replace_with`, claims the lock after the swap, keeps the asterisk, does not let the replaced editor's drop overwrite the source; t1077 (red on the model swap: media 1 is still the generator); t365–t368 green
- [ ] 3.3 Commit stage 3

## 4. The supervisor and the crash window

- [ ] 4.1 `supervisor.rs`: `classify`, spawn of `current_exe` with `KT_SUPERVISED`, PDEATHSIG on Linux, the fresh-report rule, the supervisor's own report, the `shown:` mark, the restart loop with `KT_AFTER_CRASH`; `main.rs` calls it; `KT_NO_SUPERVISOR`
- [ ] 4.2 The crash window (catalog section `[crash]` in five languages, theme, fonts, language from `KT_LANG`/`app.ron`, buttons outside the scroll, `<dump>.crash.json` under `KT_GEOM_DUMP`)
- [ ] 4.3 t1072 (a child test process panics → detected), t1073 (abort/SIGKILL/SIGTERM/clean + Windows codes), t1074 (the window's content and buttons, kittest)
- [ ] 4.4 Bench command `crash`; live scenario `src/scripts/live/crash.py` (edit → crash → the window → Restore → the recovery window → the edit is back), a screenshot per step; `recover.py` and the rest stay green
- [ ] 4.5 Commit stage 4

## 5. Close

- [ ] 5.1 `docs/guide/whats-new.md` (`## kt26.11`): the crash window, the restore that brings the right sources, the per-run log
- [ ] 5.2 Registry row K9 in `docs/ktprompts/kkt-26.11.01-index.md` → done; `python3 src/scripts/ktprompts-index.py --check` green
- [ ] 5.3 `apps/kt-app/CLAUDE.md`: the supervisor section (two processes, `KT_NO_SUPERVISOR`, PDEATHSIG, the window is not a `KtApp` window)
- [ ] 5.4 `cargo test --workspace`, `cargo clippy --all-targets`, `./scripts/finish.sh`; the version here
- [ ] 5.5 Archive the change (specs merged), commit
