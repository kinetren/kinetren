# Tasks

## 0. Records

- [x] 0.1 The owner's words and answers of 2026-10-28 in `docs/user-feedback.md` ("Richer telemetry"); verify the entry names this change

## 1. Schema, identify, instances (no UI)

- [ ] 1.1 `telemetry.rs`: per-event key table (D1), standard keys `$os`/`$os_version`/`$app_version`/`$app_build` (D3), `$process_person_profile` removed, `$set`/`$set_once` on `app_started` (D2); verify t837 (every event against its row, with the path/clip/panic traps) and t880
- [ ] 1.2 UUID v7 session id, `telemetry-seq`, `instances/<session>.lock` with `File::try_lock` and stale removal (D4); verify t881 and that t836/t841 stay green
- [ ] 1.3 Commit stage 1 (`cargo test -p kt-app` green, clippy clean on touched files)

## 2. Counters and heartbeat

- [ ] 2.1 `Counters` (atomics + small maps), `tel::bump` helpers that are no-ops without telemetry, heartbeat on the sender thread with `recv_timeout` and an injectable clock, "send only if changed", final counters in `app_closed` (D5); verify t879 and t836
- [ ] 2.2 Bump sites in `src/apps/kt-app/src/app.rs` (apply/undo/redo wrappers, media add paths, save, open/import/recovery, playback transition and input slots in `draw_all`) without allocation in the draw path; verify t884 (real UI paths, with the PATH line) and the existing `layout_tests.rs`/`reach_tests.rs`
- [ ] 2.3 Commit stage 2

## 3. Milestones, crash, environment

- [ ] 3.1 `project_opened` from the open/import/recovery/new paths and `export_finished` where `render_dialog.rs` clears `render_job`; bucket functions (D7); verify t883
- [ ] 3.2 Panic hook writes `session:`; `crash_from_report` parses version/thread/location and drops the message; `app_crashed` sent from the start hand-off next to `take_crash_report` (D8); verify t882 and that t374/t375 (`journal.rs`) stay green
- [ ] 3.3 `probe_env` over injected sources + production gatherers (`/etc/os-release`, `RtlGetVersion`, `/proc/meminfo`/`GlobalMemoryStatusEx`, adapter info from `ui()`, scale and monitor size, FFmpeg state from `ffmpeg_setup.rs`) (D9); verify t886 and that the Windows cross-build compiles (`./scripts/release-build.sh --target windows` or `make windows`)
- [ ] 3.4 Commit stage 3

## 4. Texts: bell only, spoiler, calm

- [ ] 4.1 `Notice.details` + a collapsed "Details" header in the notices window; first start posts summary + details to the bell only (no `status`); View hint = summary + not-sent (D10); verify t840 (with the updated PATH), t885, t843
- [ ] 4.2 Catalog keys in all five `src/apps/kt-app/i18n/*.toml` (calm wording, ≤ 160 chars, Arabic punctuation), `headline` removed; `python3 scripts/subset-fonts.py`; verify `font_tests.rs`, `i18n_tests.rs`
- [ ] 4.3 README "Privacy": one friendly paragraph, the lists in `<details>`, still saying what is NOT sent; verify `python3 src/scripts/readme-check.py --check`
- [ ] 4.4 Commit stage 4

## 5. Verify and finish

- [ ] 5.1 Live check against a local capture server (debug build with a test key in a separate `CARGO_TARGET_DIR`, `KT_TELEMETRY_URL`): start → import a fixture → play → close; the server holds `app_started` with `$set`, `project_opened`/counters, `app_closed` with counters; screenshots of the bell entry collapsed and expanded
- [ ] 5.2 Owner check in PostHog (listed in the report, done by the owner): Persons shows one person per install with `$os`, `$app_version`, GPU and FFmpeg properties; a dashboard for active time, instances and crash locations
- [ ] 5.3 `cargo test --workspace` and `cargo clippy --all-targets` green; `src/scripts/finish.sh`, version in the report
