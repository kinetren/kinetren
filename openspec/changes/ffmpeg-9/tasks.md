# Tasks

Test numbers t862–t867 are reserved by this change. Every stage ends with a commit (English
message, first line = stage and what was done, no `Co-Authored-By` or other agent lines).

## 0. Records before code

- [x] 0.1 Append the owner's request and decisions of 2026-10-26 to `docs/user-feedback.md` (Windows Download → 9.0.2, "CI tests 9 only"; Linux proof = 9.0.2 built from source in Docker); verify the entry names `ffmpeg-9`
- [ ] 0.2 Commit stage 0 together with the change's artifacts (`openspec validate ffmpeg-9 --strict` green)

## 1. FFmpeg 9 headers, the table and the v9 module

- [ ] 1.1 Write `src/scripts/vendor-ffmpeg-headers.sh <version> <sha256>` (design D1). Measure the SHA-256 of `https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz` (check the `.asc` signature with the FFmpeg release key once, by hand, and record that in this task). Run the script to create `src/libs/kt-ffmpeg/vendor/ffmpeg-9/include` with the five libraries' `.h`, the hand-made `avconfig.h` and `vendor/ffmpeg-9/SOURCE`. Verify `grep LIBAVCODEC_VERSION_MAJOR vendor/ffmpeg-9/include/libavcodec/version_major.h` gives 63, and that the tree has the same five directories as `ffmpeg-8`
- [ ] 1.2 `build.rs` `MAJORS` += `"9"`; `lib.rs`: `version_module!(v9, …)`, `VERSION_MODULES`, the `Backend/Source/Muxer::V9` arms, and the exhaustive match in `rescan_with` (no `_ => V6`, design D2). Verify `cargo build -p kt-ffmpeg` and `cargo check --workspace --target x86_64-pc-windows-gnu` (in the `windows` container). Fix any field or constant FFmpeg 9 removed inside the version module only, and list each such fix in this task
- [ ] 1.3 `search.rs`: the row `9: 61/7/10/63/63` first in `MAJORS`; update the "not here yet" comments; update `not_found_hint` to "6, 7, 8 or 9" (the Windows winget line follows in 4.2). Verify t844 and t845 updated per the delta (a complete set 9 in a known location; `avcodec-64.dll` → "FFmpeg 10 (avcodec 64) is not supported yet"; `avcodec-63.dll` without `avutil-61.dll` → "FFmpeg 9: no avutil-61.dll"), all in `src/libs/kt-ffmpeg/tests/loader_plan.rs`
- [ ] 1.4 Add t862 (table == `vN::ffi::LIB*_VERSION_MAJOR` for every module, modules == rows) and t863 (Linux plan: SONAMEs 9→6, `KINETREN_FFMPEG_DIR` with a complete/incomplete 9 set), both in `src/libs/kt-ffmpeg/tests/loader_plan.rs`. Verify both green, plus t10/t21/t37/t112 still green on the host FFmpeg 6.1
- [ ] 1.5 Commit stage 1 (`cargo test --workspace` and `cargo clippy --all-targets` green)

## 2. Linux proof: FFmpeg 9.0.2 built in Docker

- [ ] 2.1 Add the `ffmpeg-test` stage to `src/packaging/Dockerfile` (design D5): `ARG FFMPEG_VERSION=9.0.2`, `ARG FFMPEG_SHA256` equal to 1.1, nasm + libx264-dev, `sha256sum -c`, shared GPL+libx264 build into `/opt/ffmpeg`, nextest. The stage is not last. Verify `docker build --target ffmpeg-test` succeeds, that `ls /opt/ffmpeg/lib` shows `libavcodec.so.63`, and that a plain `docker build` still ends at `linux`
- [ ] 2.2 Write `src/scripts/ffmpeg-test.sh` and `make ffmpeg-test` (with a `##` help line). The script runs nextest for `kt-ffmpeg kt-media kt-engine kt-cli` in the stage with `KINETREN_FFMPEG_DIR=/opt/ffmpeg/lib`, `KINETREN_FFMPEG_SYSTEM=0`, `KT_EXPECT_FFMPEG_MAJOR=9`, its own target dir, and host-generated fixtures. Verify `make ffmpeg-test` runs and that the log names FFmpeg 9 (avcodec 63.x)
- [ ] 2.3 Add t865 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`): with `KT_EXPECT_FFMPEG_MAJOR` set, assert the loaded major and print the report on mismatch; unset → skip with a reason. Verify it is green in `make ffmpeg-test` and fails when run there with `KINETREN_FFMPEG_DIR` at an empty folder, and that it skips on the host
- [ ] 2.4 Make the whole `make ffmpeg-test` run green on FFmpeg 9: t10, t21, t37, t112, t33, t849 and the rest of the four packages. Any failure that comes from a changed FFmpeg behaviour is fixed in the version module. Any test that turns out to hash FFmpeg-decoded output is reported to the owner, not re-baselined (design, Risks). Record the pass count here
- [ ] 2.5 Add the CI job `ffmpeg9` to `.github/workflows/ci.yml`: ubuntu-24.04, fixtures, the Docker layer cache keyed by the Dockerfile hash, `src/scripts/ffmpeg-test.sh`, and only a failure log with `retention-days: 3`. Extend `src/scripts/dist-check.py` and add t866 (`src/apps/kt-cli/tests/dist_guard.rs`) for the stage, the hash check, the vendor-script pin == Dockerfile ARGs, the make target, the job with `KT_EXPECT_FFMPEG_MAJOR`, and no reference from `release-build.sh`. Verify t834/t835/t860/t866 green
- [ ] 2.6 Commit stage 2

## 3. Earlier downloads stay searchable

- [ ] 3.1 `kt-dirs`: `ffmpeg_download_root()`. `search::Env::download_dir` → `download_root`, and the Windows plan pushes the version-named subfolders newest first, skipping other entries (design D3). `SystemEnv`, kt-app (`ffmpeg_setup.rs`) and kt-cli (`main.rs`) pass the root. Verify t864 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`), t844/t845, and t849 (`src/apps/kt-cli/tests/cli.rs`)
- [ ] 3.2 The FFmpeg window's offer as a pure function of (report, the pin folder exists). **Download** stays offered over an earlier download, and the window names the loaded folder. Verify t867 (`src/apps/kt-app/src/ffmpeg_setup.rs`), t850 (`src/apps/kt-app/src/reach_tests.rs`), and the kt-app `reach_tests.rs`/`layout_tests.rs`/`i18n_tests.rs`/`font_tests.rs` (UI text change)
- [ ] 3.3 Commit stage 3

## 4. Windows pin 8.1.2 → 9.0.2

- [ ] 4.1 Download `ffmpeg-9.0.2-full_build-shared.7z` from gyan.dev and from the GyanD/codexffmpeg mirror. Confirm both are byte-identical, and measure the SHA-256 and size. List `bin/` to confirm `avutil-61.dll`, `swresample-7.dll`, `swscale-10.dll`, `avcodec-63.dll`, `avformat-63.dll`. Check that `winget show Gyan.FFmpeg.Shared --version 9.0.2` and scoop `ffmpeg-shared@9.0.2` exist (from the manifests' repositories). Record all of it in this task
- [ ] 4.2 Set the pin everywhere (design D4): `kt-dirs::FFMPEG_DOWNLOAD_VERSION`, `ffmpeg_setup.rs` `PINNED_*` (archive, hash, URLs, size), `src/scripts/fetch-ffmpeg-win.sh`, `src/scripts/windows-test.sh`, the windows-latest job of `ci.yml`, `loader_windows.rs` (folder `9.0.2`, major 63, decoy `avutil-61.dll`), `reach_tests.rs` (folder and the t850 path text), the Windows `not_found_hint`, and `README-windows.txt`. Extend t860 with the equality of version and hash across `ci.yml`, the fetch script and `ffmpeg_setup.rs`. Verify t848 (fixture archive now `ffmpeg-9.0.2-full_build-shared/…`, folder `9.0.2`), t860, and `grep -rn "8\.1\.2"` in src/, .github and openspec/specs, which should leave only the historical "Why" lines and the earlier-download tests
- [ ] 4.3 `make windows-test` under wine against 9.0.2: t846 loads major 63 from one folder, and the FFmpeg tests pass. Verify the pass count, recorded here, matches the previous wine run minus the skips named by reason
- [ ] 4.4 Commit stage 4. After the owner pushes, record the windows-latest and `ffmpeg9` CI results here (run ids). A Windows golden row that changes with 9.0.2 is committed only with that run's measured value and provenance line, as t859 requires

## 5. Wording, notes and specs

- [ ] 5.1 Change "6/7/8" to "6/7/8/9" in the root `CLAUDE.md` (invariant 10), `openspec/config.yaml`, `docs/architecture.md`, `CONTRIBUTING.md`, the README badge and text (Download = 9.0.2, the winget line), `src/libs/kt-ffmpeg/CLAUDE.md` (replace the "FFmpeg 9 is not supported yet" bullet with the add-a-major checklist: vendor script → `MAJORS` in build.rs → module + arms → table row → t862), `src/libs/kt-media` notes, and the Dockerfile, release-build.sh and Makefile comments. Verify `python3 src/scripts/readme-check.py --check` and `dist-check.py --check` green
- [ ] 5.2 Verify `openspec validate ffmpeg-9 --strict`, `python3 src/scripts/specs-user-path.py --check` (t799, the t850 PATH) and `python3 src/scripts/features-index.py --check` are green
- [ ] 5.3 Commit stage 5

## 6. Finish

- [ ] 6.1 `cd src && cargo test --workspace` and `cargo clippy --all-targets` green on the host. `make ffmpeg-test` and `make windows-test` are green (the results of 2.4 and 4.3, rerun after stages 3–5)
- [ ] 6.2 Run `src/scripts/finish.sh`: it rebuilds kt-app/kt-cli and prints the version `vYY.MM.DD-hash` for the report. `kt-cli ffmpeg` on the host still loads FFmpeg 6.1 by SONAME. Commit
- [ ] 6.3 After archiving: append the "why" (D1–D5, the rejected options) to `openspec/specs/media-index/design.md` and `openspec/specs/distribution/design.md`, remove the "Deferred: FFmpeg 9" line, and update the verdict of `docs/features/resilience/environment/system-libraries-and-drivers.md`. Verify `features-index.py --check` is green
