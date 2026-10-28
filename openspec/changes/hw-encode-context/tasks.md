# Tasks

Test numbers t960–t965 are reserved by this change. It is a prerequisite of `flatpak-flathub`;
t965 also needs the encoder row of `render-window-v1-parity` (apply that change first, or keep
t965's progress-line half for after it). Every stage ends with a commit (English message, the
first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's decision "Flathub + a VAAPI context" and ktprompts v26.10 T16 are recorded in `docs/user-feedback.md`; the proposal, deltas (export, media-index), design and tasks are written. Verify `openspec validate hw-encode-context --strict` is green
- [x] 0.2 F-EXP-004 (`docs/features/export/pipeline/hardware-encoder-selection.md`) gets a dated revision of its verdict naming this change. Verify `python3 src/scripts/features-index.py --check` reports no error for this note
- [ ] 0.3 Commit stage 0

## 1. Prove the crash and the QSV path (D1, D2)

- [ ] 1.1 Probe T16: `kt-cli export src/fixtures/<demo>.kt /tmp/out.mp4 h264_vaapi` under `gdb -batch -ex run -ex bt` on this VM, on the owner's machine (with `KT_EXPECT_HW` unset) and in the `ffmpeg-test` container (FFmpeg 9); the same choice through the GUI with the live rig. Record per run: signal or error text, top frames, FFmpeg major, driver — here, and name which of H1/H2/H3 holds
- [ ] 1.2 Measure `h264_qsv` with NV12 system frames on an Intel machine (owner) and record "opens / code N"; if it does not open, record the revision of D2 (a QSV context) in `design.md` before stage 3
- [ ] 1.3 Write t960 (`src/libs/kt-ffmpeg/tests/encoders.rs`), t961 (same file, resolver seam), t962 (`src/libs/kt-engine/tests/export_options.rs`), t963 (same file, gated), t964 (`src/apps/kt-cli/tests/cli.rs`, `KT_DRI_DIR`) and t965 (`src/apps/kt-app/src/layout_tests.rs`, with its `- **PATH**`). Verify they compile and are RED (t963 skipped without `KT_EXPECT_HW`), and record the first failure lines here
- [ ] 1.4 Commit stage 1 with the probe record

## 2. Types and optional symbols (D5)

- [ ] 2.1 `build.rs`: add `libavutil/hwcontext.h` to `HEADERS` for all four majors; allowlist the `AVHWFramesContext`/`AVHWDeviceType` types. Verify `cargo build -p kt-ffmpeg` and t862 green
- [ ] 2.2 `api.rs`: `declare_optional_api!` and `HwApi` with `load_with`; the version modules keep `Option<&HwApi>` and the missing-name reason; the dispatch in `lib.rs`. Verify t961 green and t10/t21/t37/t849 unchanged
- [ ] 2.3 Commit stage 2

## 3. The VAAPI path (D2–D4, D7)

- [ ] 3.1 `encoders.rs`: `HwKind` on every candidate, `h264_vaapi` = `Vaapi`/`Nv12`, its bitrate options. Verify t858 and t960 green
- [ ] 3.2 `muxer_impl.rs`: the device → frames → open step with skip reasons kept for the open report; the per-frame upload with pts and duration; the `Drop` order. Verify t962 green, t25/t33 green, and the golden stream hashes (t11, t485) unchanged
- [ ] 3.3 On the owner's machine: `KT_EXPECT_HW=vaapi make hw-test` (new Makefile target running the FFmpeg tests on the host with the variable). Verify t963 green; record the device, driver vendor string, FFmpeg major and the measured mean difference, and set the bound in D8
- [ ] 3.4 Commit stage 3

## 4. The list, the report, the window (D6)

- [ ] 4.1 `available_encoders()` with the device; `hw_report()`; `kt-cli ffmpeg` section and `--probe-encoders`. Verify t964 and t849 green
- [ ] 4.2 `render_dialog.rs`: the combo from the new list with the device; the progress row names a hardware encoder's device (needs `render-window-v1-parity`'s encoder row). Verify t965 green and the ui-reach/ui-timeline guards (`reach_tests.rs`, `layout_tests.rs`) green
- [ ] 4.3 Speed on release: `cargo build --release -p kt-cli`; a 1080p 60 s export with `libx264` and with `h264_vaapi` on the owner's machine; record both wall times here (release only)
- [ ] 4.4 `openspec/specs/export/design.md`: remove the "known remainder" about vaapi/qsv never opening from "Non-obvious details" and the matching "Deferred" item, and the media-index "Deferred" line (at archive). Verify `openspec validate --specs` is green
- [ ] 4.5 Commit stage 4 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-ffmpeg -p kt-engine -p kt-cli -p kt-app --all-targets` with no new warnings)

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` here, with the t963 record of 3.3
- [ ] 5.2 After archiving: set F-EXP-004's verdict line to the delivered state (`CAPABILITY:` unchanged), and append the "why" (D1 the probe's finding, D3 skip-before-open, D5 optional table, D8 loud opt-in proof) to `openspec/specs/export/design.md` and `openspec/specs/media-index/design.md`
