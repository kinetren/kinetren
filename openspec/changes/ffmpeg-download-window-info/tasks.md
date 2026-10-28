# Tasks

Test numbers t948–t953 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done. Depends on the archived
`ffmpeg-9` (2026-10-27, already in this branch). Its t867 and t850 must stay green throughout.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's complaint (`kkt.txt` 2026-10-27, the second item 3) is in `docs/user-feedback.md` (round of 2026-10-27), and the change's artifacts are written. F-MED-007 is `done` and stays frozen. Verify `openspec validate ffmpeg-download-window-info --strict` is green

## 1. The downloader tells everything

- [ ] 1.1 `DownloadProgress` snapshot (design D1), `DownloadSource.size`/`archive` (D3), the size check before the body, per-mirror `attempts`, the unpacking count (D4), and the result carries the folder and the verified hash. Move t848's step matching to the snapshot. Verify t848 is green unchanged in its assertions
- [ ] 1.2 Write t949 in `src/apps/kt-app/src/ffmpeg_setup.rs` (two local mirrors: size mismatch with no body read, 404 + break, cancel during unpacking). Verify it is green
- [ ] 1.3 The pure `rate`/`eta` functions and the sample ring (D2). Write t948. Verify it is green
- [ ] 1.4 Commit stage 1

## 2. The window

- [ ] 2.1 `FfmpegUi`: keep the result folder, the `os` override (D6), `test_open` (D5), and the sample ring fed in `poll`. `util::open_in_file_manager`. Verify `cargo build -p kt-app` and t867 are green
- [ ] 2.2 `draw_ffmpeg_window`: the "what happens" block in the body, the progress line with host, mirror, MB, speed and time left, the "Tried:" list, the success and failure blocks, and **Open folder** in the actions row (D7). Verify t850 and t779 are green
- [ ] 2.3 `[ffmpeg]` keys in all five catalogs, then `python3 scripts/subset-fonts.py`. Write t953's checks in `i18n_tests.rs`/`font_tests.rs`. Verify t953, t100, t125, t128 and t171 are green
- [ ] 2.4 Write t950, t951 and t952 in `src/apps/kt-app/src/reach_tests.rs` (harness, Windows rules forced, local server, temp download root). Verify they are green and `src/scripts/specs-user-path.py --check` (t799) accepts their PATH lines
- [ ] 2.5 Run the UI guards (`reach_tests.rs` incl. t757, `layout_tests.rs`). Verify they are green
- [ ] 2.6 Commit stage 2 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-app --all-targets` with no new warnings)

## 3. Live and Windows

- [ ] 3.1 Live scenario `src/scripts/live/ffmpeg_download.py` on Xvfb: a debug build with `KT_FFMPEG_DOWNLOAD_URL`/`_SHA256`/`_SIZE` pointing at a local server serving the t848-shaped archive, and the Windows offer forced by the test seam. The archive comes from the t848 builder; if exposing that builder to the script costs more than this stage, the live run covers only the before state and the failed state (404, then size mismatch), and the success state stays with t952. Take one `kt_live.step` screenshot per PATH step of t950–t952. Verify `./scripts/live-check.sh ffmpeg_download` is green
- [ ] 3.2 `./scripts/windows-test.sh`: the kt-app tests under wine, including t949–t952. Verify they are green. The real-Windows click on the pinned gyan.dev archive (Explorer opening the folder) stays with the owner: name it in the report
- [ ] 3.3 Commit stage 3

## 4. Records and finish

- [ ] 4.1 Append the "why" to `openspec/specs/media-index/design.md` (snapshot step, size check before the body) and `openspec/specs/ui-reach/design.md` (the window's three states, Open folder). Verify `openspec validate --specs` is green
- [ ] 4.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the live screenshots here
