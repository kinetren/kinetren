# Tasks

Precondition: the files of `github-ci-docker-build` (Dockerfile, Makefile, workflows,
`dist-check.py`) are committed; that change is archived BEFORE this one, because this change's
`distribution` delta adds to the capability it creates. Test numbers t844–t861 are reserved by this
change.

## 0. Records before code

- [x] 0.1 Append the owner's words and decisions of 2026-10-22 to `docs/user-feedback.md`: Windows build; find the installed FFmpeg plus the in-program download of gyan.dev's shared release build; cross-build with mingw in Docker; portable zip. Also record the found fact that gyan.dev's "latest" is FFmpeg 9.0.2, so the download is pinned to 8.1.2. Verify the entry names `windows-build`
- [x] 0.2 Feature note in `docs/features/` for "find or download the codec library from inside the program" (Audacity *Libraries → FFmpeg: Locate / Download*; how other editors that use system FFmpeg on Windows behave) with `CAPABILITY: openspec/specs/media-index/`, plus a `gyan.dev` row in `docs/features/_services.md`; verify `python3 src/scripts/features-index.py --check` is green
- [x] 0.3 Commit stage 0

## 1. The tree compiles for Windows

- [x] 1.1 Add the `windows` stage to `src/packaging/Dockerfile` (mingw-w64, clang, target `x86_64-pc-windows-gnu` for the pinned toolchain, WineHQ wine for jammy) and the `[target.x86_64-pc-windows-gnu] linker` line to `src/.cargo/config.toml`; verify `docker build --target windows` succeeds and t834 stays green
- [x] 1.2 `kt-ffmpeg/build.rs` decides from `CARGO_CFG_TARGET_OS`/`TARGET` only; minimal `cfg` split of `loader.rs` (unix keeps `libloading::os::unix`, windows gets a stub returning `NotFound`); verify `cargo check --workspace --target x86_64-pc-windows-gnu` in the container passes, and fix any enum signedness or `c_long` casts it reveals without touching vendored headers
- [x] 1.3 `src/scripts/release-build.sh --target windows` + `make windows`: build kt-app/kt-cli for the target in the dist profile, zip `kinetren.exe`, `kt-cli.exe`, `README-windows.txt` (a first draft) and licences, write `.sha256`; PE import check (design D10) inside the container, so a failure leaves no zip; verify `make windows` produces the zip and the check prints the import list with no forbidden names
- [x] 1.4 Commit stage 1 (`cargo test` on Linux green; the Linux tarball build unchanged)

## 2. FFmpeg loader for Windows

- [x] 2.1 Split `loader.rs` into pure `plan(env)`, per-OS `open`, and `report` (design D2): Windows DLL table for 6/7/8, directory order, static-build and unsupported-major reasons, winget `Links` dereference, known locations, `KINETREN_FFMPEG_DIR` on both OSes; verify t844 and t845 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`) on Linux
- [x] 2.2 Windows `open`: `load_with_flags(full path, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` in dependency order, `GetModuleFileNameW` check that all five lie in the chosen directory; verify t846 (`src/libs/kt-ffmpeg/tests/loader_windows.rs`) under wine with the pinned build fetched into the container cache (`src/target/ffmpeg-win/8.1.2`, SHA-256 checked)
- [x] 2.3 Retryable backend: `OnceLock<Loaded>` plus the stored failure report, `rescan()`, `backend()` never touches the disk after a failure; the error text hints per OS; verify t847, and t10/t21/t37 stay green on Linux
- [x] 2.4 The user's choice in `<data_dir>/ffmpeg-dir` (read by the plan as step 1); `kt-cli ffmpeg` prints the report and exits non-zero when nothing loads; verify t849 (`src/apps/kt-cli/tests/cli.rs`)
- [x] 2.5 Commit stage 2

## 3. Per-user directories, lock, atomic replace

- [x] 3.1 New crate `src/libs/kt-dirs` (`data_dir`, `local_dir`, injected `Env`, `SHGetKnownFolderPath` under `cfg(windows)`), with no `"."` fallback; kt-app `journal.rs` and kt-cli `kt_lock_holder` use it; the kinotrain migration is compiled only on Linux and runs only on a resolved root; verify t851 (`src/libs/kt-dirs/tests/dirs.rs`) and that t830/t831 stay green
- [x] 3.2 Pass `NativeOptions::persistence_path` from `kt-dirs`; verify on Linux that the eframe storage file path is byte-identical to before (print both in a test or a one-off check recorded here), so no settings move
- [x] 3.3 `project_lock.rs`: `std::fs::File::try_lock` on both OSes, pid in `<key>.pid`, pure `lock_key(path, flavor)`; remove the `libc` flock code and the `cfg(not(unix))` stub; verify t362/t362b/t363/t364 green and t852 (`src/apps/kt-app/src/project_lock.rs`) on Linux, and t852's Busy-with-pid part under wine
- [x] 3.4 `kt-otio io.rs`: `replace` in the file-system trait, `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` on Windows, the pure retry policy (design D6), `dir_synced = true` after write-through; journal rotation and `user_store` use it; verify t853 (`src/libs/kt-otio/tests/io.rs`) and that t351/t353 stay green
- [x] 3.5 Commit stage 3

## 4. Media paths across operating systems

- [x] 4.1 `url.rs` with `PathFlavor` (design D7): Windows file URLs (drive, UNC, `localhost`), relativising with "only drive/share shared → absolute URL", case-insensitive comparison, verbatim prefix removal; verify t854 and t855 (`src/libs/kt-otio/tests/format.rs`)
- [x] 4.2 Absolute-anywhere rule on reading (`url.rs`, `kt-otio/src/lib.rs` resolution, `kt-app/src/util.rs`): foreign absolute paths stay verbatim and become placeholders labelled with the path; relative paths get native separators before the join; verify t856 and that t147/t797/t797b stay green (t147's absolute fixtures made flavor-aware)
- [x] 4.3 Commit stage 4

## 5. In-program FFmpeg setup (Windows)

- [ ] 5.1 Add `ureq` (rustls, webpki-roots, proxy from env; skip if `telemetry-posthog` already added it), `sevenz-rust2` and `sha2` to kt-app; verify `cargo tree -p kt-app -i openssl-sys` finds nothing and no libs/ crate gained a dependency
- [ ] 5.2 Confirm the pinned SHA-256 by downloading `ffmpeg-8.1.2-full_build-shared.7z` from gyan.dev, and check whether the GitHub mirror's file has the same hash (commit both hashes with URLs if not); record the results here; verify `sevenz-rust2` extracts the real archive's `bin/*.dll` byte-identical to `7z x`
- [ ] 5.3 `src/apps/kt-app/src/ffmpeg_setup.rs`: download `BgJob` (stream + hash, extract only `*/bin/*.dll` and `*/LICENSE*` into `.partial-<pid>`, rename to `8.1.2`, cleanup on any failure or cancel, stale-pid cleanup), debug-only URL/hash override; fixture `src/fixtures/ffmpeg-mini.7z` from `src/fixtures/gen-ffmpeg-mini.sh`; verify t848
- [ ] 5.4 *View → FFmpeg…* window (report, Download with source/licence confirmation, progress and Cancel, Locate folder…, Rescan; Linux: Rescan and the distro hint) following the window-form contract, plus the once-per-session "FFmpeg not found" notice with the same buttons. After a successful load, placeholders caused by "FFmpeg not loaded" recover through the `spawn_media_watchdog` swap. Strings in all five catalogs `src/apps/kt-app/i18n/{en,ru,es,ar,zh-Hans}.toml`. Verify t850 (`reach_tests.rs`, with the PATH line), t779 (window-form guard), `font_tests.rs`, `i18n_tests.rs`, `layout_tests.rs`
- [ ] 5.5 Commit stage 5

## 6. Audio output, encoders, small Windows behaviour

- [ ] 6.1 `audio_out.rs` `OutputEdge` (Direct / Map / Resample; rubato fixed-in with a preallocated FIFO; playhead in engine frames; resampler delay in latency compensation); verify t857 (counting allocator) and that the transport tests listed by `openspec show transport --type spec` stay green
- [ ] 6.2 `video_encoder_candidates(os)` with per-candidate pixel format and quality options; QSV via NV12; vaapi only on Linux; amf and mf on Windows; verify t858 (`src/libs/kt-ffmpeg/tests/encoders.rs`) and t25/t33 green on Linux
- [ ] 6.3 Locale from `GetUserDefaultLocaleName` behind an injectable source; verify t861 (`src/apps/kt-app/src/i18n_tests.rs`)
- [ ] 6.4 Windows `dnd_pointer_pos` (`GetCursorPos` + `ScreenToClient`), `windows_subsystem = "windows"` for release, `src/packaging/kinetren.ico` plus the version resource via `winresource` in kt-app `build.rs` (target-gated), WARP in the software-adapter names; verify the zip's exe shows the icon (`x86_64-w64-mingw32-objdump -x` lists the resource) and record the manual Windows check (drop on the second lane at about 10 s, no console, icon and version in Properties) here
- [ ] 6.5 Forced panic in the Windows dist exe under wine: the crash report holds function names; if it doesn't, apply the D1 fallback (`strip = "none"` + `debug = 1` for the Windows dist target only) and re-check
- [ ] 6.6 Commit stage 6

## 7. Tests on Windows, CI and release

- [ ] 7.1 `.gitattributes` (`* text=auto eol=lf`, binary for media/`.db`/`.7z`/`.png`/`.ico`); portable forms for `/proc/self/…` unwritable paths, `/tmp/…` literals and source-scanner separators; python/bash/Xvfb-dependent tests skip on Windows with a named reason; verify `cargo test --workspace` stays green on Linux and the skips print their reasons under wine
- [ ] 7.2 `make windows-test`: nextest archive for the Windows target with the fixtures (`.config/nextest.toml` profile `ci-windows`), run under wine in the container with the pinned FFmpeg; record which tests fail only under wine and why
- [ ] 7.3 `ci.yml`: the Linux job builds and uploads the archive (`retention-days: 1`); the `windows-latest` job downloads it plus the pinned FFmpeg (hash-checked) and runs `cargo nextest run --archive-file … --workspace-remap .`, uploading only a failure log (`retention-days ≤ 3`). `release.yml` builds and attaches the zip and `.sha256`, and creates no release if either build fails. Extend `dist-check.py`; verify t860 (`src/apps/kt-cli/tests/dist_guard.rs`) and t834/t835 green, and `actionlint` when available
- [ ] 7.4 Golden tables keyed by `(os, arch)` with a provenance line (golden_export, title_golden incl. t662, video_fx, fx_thumb, geo_fuse); after the first green Windows CI run, commit the Windows values with the run's id; verify t859 and t11/t485 on Linux, and the same tests green on the Windows job
- [ ] 7.5 Commit stage 7

## 8. Docs and finish

- [ ] 8.1 Root `CLAUDE.md`: invariant 10 wording (Linux `ldd`, Windows PE imports) and the `make windows` / `make windows-test` lines; `src/libs/kt-ffmpeg` and `kt-dirs` crate `CLAUDE.md` (capability line, the D2 loading rule as a rule for writing code); README (Windows install: unzip, then Download in the program or `winget install Gyan.FFmpeg.Shared --version 8.1.2`; SmartScreen note); final `README-windows.txt`; `CONTRIBUTING.md` (cross-build, wine tests); verify `readme-check.py --check` and `dist-check.py --check`
- [ ] 8.2 Manual check on a clean Windows 10/11 x64 recorded here: unzip, start `kinetren.exe`, see the notice, Download, the clip plays, export picks a hardware encoder or libx264 (name from the export stats), save, then save again with the project open in an editor (retry path), and a second instance shows Busy with a pid
- [ ] 8.3 `cargo test --workspace` and `cargo clippy --all-targets` green; `src/scripts/finish.sh`; version `vYY.MM.DD-hash` in the report
- [ ] 8.4 After the owner's first tag with this change: the release carries both assets and `sha256sum -c` passes on both (record here); then archive the change, update the feature note's verdict (`CAPABILITY:`), and add the "why" to `media-index`, `durability`, `project-format`, `export`, `transport` and `distribution` `design.md` (design D2–D11, with the revision items)
