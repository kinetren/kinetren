# Tasks

Test numbers t954–t959 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's words (kkt item 4) and the R.71 items U1/T12 are recorded in `docs/user-feedback.md` ("The owner's list of 2026-10-27 …"); the proposal, delta spec, design and tasks are written. Verify `openspec validate render-window-v1-parity --strict` is green
- [x] 0.2 Feature note F-EXP-005 `docs/features/export/pipeline/render-progress-window.md` (reconnaissance of the five editors, `CAPABILITY: openspec/specs/export/`). Verify `python3 src/scripts/features-index.py --check` is green once the `export/` row of `docs/features/README.md` reads 5 notes and the summary table carries F-EXP-005
- [x] 0.3 Commit stage 0
  - Stage 0 was committed with the proposal (944a3cf).

## 1. Tests first

- [x] 1.1 Write t954 and t955 in `src/libs/kt-engine/tests/export_options.rs` against a placeholder `ExportEvent`/`export_mp4_with_backends` event signature (D1). Verify they compile and are RED, and record the first failure line here
  - t954 and t955 in `export_options.rs`, against the real `ExportEvent` (written with it, not a
    placeholder signature). t954: forced `libx264` and the cascade — the first non-log event is the
    report, before every video step; libx264 = software, yuv420p, `crf` among the options; the report's
    `tried` equals the "not opened" log lines and follows `kt_media::muxer::encoder_cascade()` (this
    machine: nvenc, vaapi, qsv, each with its code); bytes never decrease, end > 0, sum ≤ the file.
    t955: two libx264 runs give equal streams. RED with the report not sent: "forced: the first non-log
    event is not the encoder report: Progress(… phase: Video …)".
- [x] 1.2 Write t956, t957, t959 in `src/apps/kt-app/src/layout_tests.rs` (a scripted `RenderJob` fed `RenderMsg`s, the `Clock` seam of D3) and t958 in `src/apps/kt-app/src/reach_tests.rs` (`KT_FILE_PICK` at a temp file), each with the spec's `- **PATH**`. Verify they are RED and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH lines
  - t956, t957, t959 in `layout_tests.rs` drive a `RenderJob` whose thread only waits for Cancel;
    the test hands it `RenderMsg`s through `on_msg`/`on_result` (the path the thread's messages take)
    and moves a thread-local scripted clock (`render_clock`, the `fn() -> Instant` seam of D3). t958
    in `reach_tests.rs` answers the save step through `RenderSetup.test_pick`, not `KT_FILE_PICK`: the
    harness runs many tests in one process, and an environment variable would answer for all of them.
    The PATH lines of the spec are accepted by `specs-user-path.py --check`. Red proofs (each fix
    disabled in turn): t956 "the window does not show “h264_nvenc (hardware) · yuv420p · CQ 23”"
    (the CQ option read as CRF), t957 "elapsed moved at 11 s" (elapsed read from the clock after the
    finish), t958 "no overwrite question" (the exists check off), t959 "the window's log is not
    bounded" (the 800-line ring off).

## 2. The engine reports

- [x] 2.1 `muxer_impl.rs`: keep every failed candidate with its code and the opened candidate's options; `open_report()`; per-stream packet byte counters in `drain`; expose both through the version dispatch in `src/libs/kt-ffmpeg/src/lib.rs` and `kt-media::muxer`. Verify t858 and t25 stay green
  - `kt_ffmpeg::OpenReport { encoder, hardware, pix_fmt, options, tried }`; `muxer_impl` keeps every
    failed candidate with its code and the opened one's options; packet bytes per stream in `drain`
    before `av_interleaved_write_frame`; `Muxer::open_report()`/`bytes()` through the version dispatch,
    re-exported by `kt_media::muxer` (with `encoder_cascade()` for the test). t858, t25 green.
- [x] 2.2 `export.rs`: `ExportEvent` (D1), `Opened` before the video phase, bytes in every `Progress`, `Log` facts (not-opened candidates, placeholders from `open_all`'s list passed in, phases, finalizing, errors); `export_mp4_with` filters to progress. Verify t954, t955 green and t11/t33/t116 unchanged
  - `ExportEvent = Opened | Progress | Log`, `ExportLog` facts (NotOpened, Placeholder, Phase,
    Finalizing, Error); `ExportProgress` gains `video_bytes`/`audio_bytes` (stored from the muxer after
    each write); `ExportOptions.placeholders` (the caller's list — export cannot tell a placeholder);
    errors are logged before returning (a cancel is not an error); `export_mp4_with` filters to
    progress, its four tests unchanged. t954, t955, t11, t33, t116 green.
- [x] 2.3 `kt-cli export` prints the log facts to stderr in English. Verify `src/apps/kt-cli/tests/cli.rs` stays green and a manual `kt-cli export … libx264` shows the opened-encoder line
  - `kt-cli export` prints the report and the facts in English on stderr. By hand on a 960×540
    project: "encoder: libx264 (software, yuv420p) preset=veryfast crf=23", "phase: video", "phase:
    audio", "finalizing the file", "done". cli.rs green.
- [x] 2.4 Commit stages 1–2
  - Committed. The render dialog takes the progress steps out of the events (a minimal bridge; stage
    3 rewrites it). Workspace: kt-app green except t640 (load flake); clippy: only the pre-existing
    collapsible `if` in kt-cli.

## 3. The window

- [x] 3.1 `bg.rs`: `poll_with` (D4); `RenderMsg`; the job thread sends the backend line (`export_video_gpu` returns its reason, no `eprintln!`) and the placeholder list. Verify the other `BgJob` users' tests stay green
  - `BgJob::poll_with` hands every message to a visitor (`poll` keeps the last one for the other
    users); `BgJob.started` is gone — the UI's clock lives in `RenderJob` (D3). `RenderMsg = Engine |
    Backend`; the job thread sends `Backend(Gpu(adapter) | Cpu(reason))` first: `export_video_gpu`
    returns `Result<_, String>` (`KT_VIDEO_GPU=0` is named as the reason), the `eprintln!` is gone.
    The placeholder list travels in `ExportOptions.placeholders` (stage 2). t398 green.
- [x] 3.2 `render_dialog.rs`: `window_body`/`window_actions`; the Output and Run forms; the speed line + `RollingGraph` chart; size, estimate, bitrate (D3, D5). Verify t956 green and t779 (form contract) green
  - `draw_render_dialog`: `window_body` holds the Output and Run forms (a two-column `Grid` each,
    side by side when the window is at least 600 pt wide, else one under the other), the speed line
    and the `RollingGraph` chart (`render.speed_graph`); `window_actions` holds the bar and the
    buttons. The video row is the opened report — "choosing the encoder…" until it arrives — with
    the quality under the encoder's own name (`quality_text`: CRF/CQ/QP/global quality, or the target
    bitrate). Size so far, the estimate from the fraction done, the bitrate of the last ≥ 0.5 s
    sample (bytes per second of OUTPUT). The window opens in the t779 table now (a scripted scene,
    `render_progress_scene`) instead of "cannot be opened without a codec". t956, t779 green.
- [x] 3.3 Freeze at the finish, the final block, Cancel disabled in Finalize with its hint, Open file / Open folder / Open log, Close away from Cancel's rect; the `os_open` helper (D6, D7). Verify t957 green and the t38 family (no repaint when idle) green
  - `on_result` stops `finished_at`, reads the file size once; the final block (time, × real time,
    frames, encoder, backend, MB, Mbit/s) and the audio count, then the loudness/missing/lost lines.
    Success offers Open file / Open folder, failure — Open log; Close is laid out from the right edge,
    away from Cancel's rect (kept in `cancel_rect`). Cancel is disabled in Finalize; its hint is set
    from `contains_pointer()` (egui's `hovered()` is always false on a disabled widget). The OS helper
    is the existing `util::open_in_file_manager` (xdg-open / explorer.exe), a test records the path
    in `RenderSetup.test_open`; a failure is a warn naming the path. The window repaints every
    100 ms only while the job runs. t957 green; t38 family green in the kt-app run.
- [x] 3.4 `Ask::OverwriteRender` (D8): asked after `pick_save`, Replace / Choose another name / back to settings. Verify t958 green and t757 (no new window) green
  - `Ask::OverwriteRender { path }`: `launch_render` asks when the picked path exists (settings
    closed); the question names the file, MB and the modification time (`journal::stamp_of`), the
    full path, and "the name can be chosen again in Render…" (rule 19). Replace → `start_render_to`,
    Choose another name → `launch_render` (the save step again), closing → the settings window back
    with its values. Probes `ask.replace`, `ask.other_name`. t958 green, t757 green (the question is
    the `kt_ask` modal).
- [x] 3.5 The log ring and `render.log` / `render.log.1` (D9); the path in the window; the read-only fallback. Verify t959 green
  - `LogLine` (English for the file, translated for the window) in a ring of `LOG_LINES = 800`
    stamped with the run's elapsed time; `LogFile` moves the previous `render.log` to
    `render.log.1` and flushes at phase changes and at the end; an unwritable directory gives one
    journal line and `failed`, the window then says the log is only there. The section is a
    `CollapsingHeader`, forced open after an error. `RenderSetup.log_dir` is the data directory (a
    temp folder under `cfg!(test)`, so a test never rotates the developer's log). t959 green; its
    read-only part is skipped when the process can write a 0555 folder (root).
- [x] 3.6 Catalogs: the new `[render]` keys in all five `i18n/*.toml`; `python3 scripts/subset-fonts.py`. Verify t959's catalog part and the existing guards t100, t125, t128, t171 are green
  - 47 new `[render]` keys in the five catalogs (Arabic with `،`/`؟`, every value ≤ 160 chars);
    the six keys of the old window removed (`summary`, `file`, `encoder_default`, `phase`,
    `counters`, `done_msg`). Format-only rows (`"{} ({}) · {} · {}"`, `"MP4 (+faststart)"`,
    `"{} / {}"`) are built in code, not catalogued. `subset-fonts.py`: the SC subset 779 codepoints.
- [x] 3.7 Run the kt-app UI guards (`reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`, `i18n_tests.rs`). Verify green, naming any failure that belongs to another open change
  - kt-app: t919 and t922 assumed every openable window closes on Esc/cross; the progress window
    became openable (t779 table, scripted scene) and its kind says the opposite. They now check the
    opposite for `PROGRESS_WINDOWS` (Esc and a click where a cross would stand leave it open) —
    stricter, not weaker. t804 (live coverage) was red at 92 untouched places until the live
    scenario `render` (4.1) existed: 81 of 81 with it. Everything else green; t815 and t802 failed
    once under the full workspace load and are green alone (known load flakes).
- [x] 3.8 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-app -p kt-engine -p kt-ffmpeg --all-targets` with no new warnings)
  - `cargo test --workspace --no-fail-fast`: green except t815/t802 under load (green alone).
    clippy on kt-app/kt-engine/kt-ffmpeg: no new warning (`RenderJob::new` takes the
    `too_many_arguments` allow, as 50 other places). Committed together with stage 4: t804 needs the
    scenario on disk.

## 4. Live and records

- [x] 4.1 Live scenario `render` (new, in `src/scripts/live/`; once it exists the spec's TEST lines name its path) on OUR fixture project: `KT_FILE_PICK` at an existing file in the rig's temp folder; `kt_live.step` screenshots: the settings window, the overwrite question, the progress window mid-run (both forms, the chart), the log section opened, the finished window (frozen numbers, Open file/Open folder). Verify `./scripts/live-check.sh render` is green and the screenshots are listed
  - `src/scripts/live/render.py` (OWN_PROCESS, `KT_FILE_PICK` at `kt-demo-40s.mp4` in the
    scenario's folder, a 40 s synthetic ramp at 1280×720): 8 steps — settings, overwrite question,
    the question again after another name, running (both forms, chart), log opened, paused, done
    (summary, Open file/Open folder hovered, Close at the right), closed (the file is a new MP4).
    Its first two runs were RED and found what the harness could not (design D10): the window
    widened past the screen by the output path and lost Close, "179 / 180" and 99 % at the finish,
    "0.0 MB"/"0.0 Mbit/s" for a small file, "Done" twice in the log; the first fixture rendered
    in under a second. Fixed; `./scripts/live-check.sh render` — 1 of 1 green. Screenshots:
    `target/live-out/render/01-step-settings.png` … `08-step-closed.png`.
- [ ] 4.2 `openspec/specs/export/design.md` Deferred: strike "name the encoder in the progress line" and "a line export runs on GPU/CPU" as done by this change (after archive). Verify `openspec validate --specs` is green
- [x] 4.3 Commit stage 4
  - Committed with stage 3 (one commit: the coverage guard t804 needs the scenario).

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots of 4.1 here
- [ ] 5.2 After archiving: set F-EXP-005 to `done` with its frozen verdict and `CAPABILITY:`, and append the "why" (D1 events not state, D2 packet bytes, D3 clocks in the UI, D8 always ask, the owner's v1-level request) to `openspec/specs/export/design.md`
