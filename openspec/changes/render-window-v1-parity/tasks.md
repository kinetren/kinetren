# Tasks

Test numbers t954–t959 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's words (kkt item 4) and the R.71 items U1/T12 are recorded in `docs/user-feedback.md` ("The owner's list of 2026-10-27 …"); the proposal, delta spec, design and tasks are written. Verify `openspec validate render-window-v1-parity --strict` is green
- [x] 0.2 Feature note F-EXP-005 `docs/features/export/pipeline/render-progress-window.md` (reconnaissance of the five editors, `CAPABILITY: openspec/specs/export/`). Verify `python3 src/scripts/features-index.py --check` is green once the `export/` row of `docs/features/README.md` reads 5 notes and the summary table carries F-EXP-005
- [ ] 0.3 Commit stage 0

## 1. Tests first

- [ ] 1.1 Write t954 and t955 in `src/libs/kt-engine/tests/export_options.rs` against a placeholder `ExportEvent`/`export_mp4_with_backends` event signature (D1). Verify they compile and are RED, and record the first failure line here
- [ ] 1.2 Write t956, t957, t959 in `src/apps/kt-app/src/layout_tests.rs` (a scripted `RenderJob` fed `RenderMsg`s, the `Clock` seam of D3) and t958 in `src/apps/kt-app/src/reach_tests.rs` (`KT_FILE_PICK` at a temp file), each with the spec's `- **PATH**`. Verify they are RED and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH lines

## 2. The engine reports

- [ ] 2.1 `muxer_impl.rs`: keep every failed candidate with its code and the opened candidate's options; `open_report()`; per-stream packet byte counters in `drain`; expose both through the version dispatch in `src/libs/kt-ffmpeg/src/lib.rs` and `kt-media::muxer`. Verify t858 and t25 stay green
- [ ] 2.2 `export.rs`: `ExportEvent` (D1), `Opened` before the video phase, bytes in every `Progress`, `Log` facts (not-opened candidates, placeholders from `open_all`'s list passed in, phases, finalizing, errors); `export_mp4_with` filters to progress. Verify t954, t955 green and t11/t33/t116 unchanged
- [ ] 2.3 `kt-cli export` prints the log facts to stderr in English. Verify `src/apps/kt-cli/tests/cli.rs` stays green and a manual `kt-cli export … libx264` shows the opened-encoder line
- [ ] 2.4 Commit stages 1–2

## 3. The window

- [ ] 3.1 `bg.rs`: `poll_with` (D4); `RenderMsg`; the job thread sends the backend line (`export_video_gpu` returns its reason, no `eprintln!`) and the placeholder list. Verify the other `BgJob` users' tests stay green
- [ ] 3.2 `render_dialog.rs`: `window_body`/`window_actions`; the Output and Run forms; the speed line + `RollingGraph` chart; size, estimate, bitrate (D3, D5). Verify t956 green and t779 (form contract) green
- [ ] 3.3 Freeze at the finish, the final block, Cancel disabled in Finalize with its hint, Open file / Open folder / Open log, Close away from Cancel's rect; the `os_open` helper (D6, D7). Verify t957 green and the t38 family (no repaint when idle) green
- [ ] 3.4 `Ask::OverwriteRender` (D8): asked after `pick_save`, Replace / Choose another name / back to settings. Verify t958 green and t757 (no new window) green
- [ ] 3.5 The log ring and `render.log` / `render.log.1` (D9); the path in the window; the read-only fallback. Verify t959 green
- [ ] 3.6 Catalogs: the new `[render]` keys in all five `i18n/*.toml`; `python3 scripts/subset-fonts.py`. Verify t959's catalog part and the existing guards t100, t125, t128, t171 are green
- [ ] 3.7 Run the kt-app UI guards (`reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`, `i18n_tests.rs`). Verify green, naming any failure that belongs to another open change
- [ ] 3.8 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-app -p kt-engine -p kt-ffmpeg --all-targets` with no new warnings)

## 4. Live and records

- [ ] 4.1 Live scenario `render` (new, in `src/scripts/live/`; once it exists the spec's TEST lines name its path) on OUR fixture project: `KT_FILE_PICK` at an existing file in the rig's temp folder; `kt_live.step` screenshots: the settings window, the overwrite question, the progress window mid-run (both forms, the chart), the log section opened, the finished window (frozen numbers, Open file/Open folder). Verify `./scripts/live-check.sh render` is green and the screenshots are listed
- [ ] 4.2 `openspec/specs/export/design.md` Deferred: strike "name the encoder in the progress line" and "a line export runs on GPU/CPU" as done by this change (after archive). Verify `openspec validate --specs` is green
- [ ] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots of 4.1 here
- [ ] 5.2 After archiving: set F-EXP-005 to `done` with its frozen verdict and `CAPABILITY:`, and append the "why" (D1 events not state, D2 packet bytes, D3 clocks in the UI, D8 always ask, the owner's v1-level request) to `openspec/specs/export/design.md`
