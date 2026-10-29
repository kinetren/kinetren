# Tasks

Test numbers t960–t965 are reserved by this change. It is a prerequisite of `flatpak-flathub`;
t965 also needs the encoder row of `render-window-v1-parity` (apply that change first, or keep
t965's progress-line half for after it). Every stage ends with a commit (English message, the
first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's decision "Flathub + a VAAPI context" and ktprompts v26.10 T16 are recorded in `docs/user-feedback.md`; the proposal, deltas (export, media-index), design and tasks are written. Verify `openspec validate hw-encode-context --strict` is green
- [x] 0.2 F-EXP-004 (`docs/features/export/pipeline/hardware-encoder-selection.md`) gets a dated revision of its verdict naming this change. Verify `python3 src/scripts/features-index.py --check` reports no error for this note
- [x] 0.3 Commit stage 0
  - Stage 0 was committed with the proposal (7e1917f).

## 1. Prove the crash and the QSV path (D1, D2)

- [ ] 1.1 Probe T16: `kt-cli export src/fixtures/<demo>.kt /tmp/out.mp4 h264_vaapi` under `gdb -batch -ex run -ex bt` on this VM, on the owner's machine (with `KT_EXPECT_HW` unset) and in the `ffmpeg-test` container (FFmpeg 9); the same choice through the GUI with the live rig. Record per run: signal or error text, top frames, FFmpeg major, driver — here, and name which of H1/H2/H3 holds
  - **This VM** (QEMU VGA `1234:1111`, no render node, FFmpeg 6.1.1): `kt-cli export demo.kt out.mp4
    h264_vaapi` under `gdb -batch -ex run -ex bt` — no signal, "no video encoder could be opened (last
    error: h264_vaapi: code -22)", exit 1. **FFmpeg 9** (`ffmpeg-test` container, no `/dev/dri`, no
    gdb in the image): the build has no `h264_vaapi` at all, and the error was "(last error: )" —
    an EMPTY reason (fixed: "h264_vaapi: not in this FFmpeg build"). The GUI path runs the same
    `Muxer::open`. Neither machine crashes, so T16 needs a machine with a render node and a VAAPI
    driver: H1 (failed init on a real device) or H2 (the driver) — the run on the owner's machine
    is still to be recorded here. D3 removes H1 by construction (no device → no `avcodec_open2`).
- [ ] 1.2 Measure `h264_qsv` with NV12 system frames on an Intel machine (owner) and record "opens / code N"; if it does not open, record the revision of D2 (a QSV context) in `design.md` before stage 3
  - Needs an Intel machine (the owner's); this VM's `h264_qsv` fails with "Error creating a MFX
    session: -9" (no device), which says nothing about NV12. D2 is unrevised until then.
- [x] 1.3 Write t960 (`src/libs/kt-ffmpeg/tests/encoders.rs`), t961 (same file, resolver seam), t962 (`src/libs/kt-engine/tests/export_options.rs`), t963 (same file, gated), t964 (`src/apps/kt-cli/tests/cli.rs`, `KT_DRI_DIR`) and t965 (`src/apps/kt-app/src/layout_tests.rs`, with its `- **PATH**`). Verify they compile and are RED (t963 skipped without `KT_EXPECT_HW`), and record the first failure lines here
  - Written with the code of stages 2–4 in one pass (the API they test did not exist): t960, t961
    (`kt-ffmpeg/tests/encoders.rs`), t962, t963 (`kt-engine/tests/export_options.rs`), t964
    (`kt-cli/tests/cli.rs`), t965 (`layout_tests.rs`, PATH in the spec). Red proofs: t962 "run 0:
    the VAAPI encoder was opened without a device" with the device step disabled (the candidate
    went straight to `avcodec_open2`, the old path); t965 "the device entry is not offered" with
    the device left out of the combo label; t960/t961/t964 do not compile / have no section on the
    old code. t858's expectation for `h264_vaapi` moved from YUV420P to NV12 — what this change's
    spec (t960) asks. t962 drives the device through the THREAD-LOCAL `hw_seam`, not
    `KINETREN_VAAPI_DEVICE`: the test binary runs exports in parallel, and the variable would steer
    all of them; t964 (a child process) uses `KT_DRI_DIR` as specified. t963 skipped here (no
    `KT_EXPECT_HW`).
- [x] 1.4 Commit stage 1 with the probe record
  - Committed with stages 2–4 (one commit: the tests and the code they test).

## 2. Types and optional symbols (D5)

- [x] 2.1 `build.rs`: add `libavutil/hwcontext.h` to `HEADERS` for all four majors; allowlist the `AVHWFramesContext`/`AVHWDeviceType` types. Verify `cargo build -p kt-ffmpeg` and t862 green
  - `libavutil/hwcontext.h` in `HEADERS` for the four majors (the `AV.*` allowlist already takes
    `AVHWFramesContext`, `AVHWDeviceContext`, `AVBufferRef`). t862 green.
- [x] 2.2 `api.rs`: `declare_optional_api!` and `HwApi` with `load_with`; the version modules keep `Option<&HwApi>` and the missing-name reason; the dispatch in `lib.rs`. Verify t961 green and t10/t21/t37/t849 unchanged
  - `ffmpeg_hw_api!` / `declare_optional_api!` build `HwApi` (7 functions) with `load_with(resolver)`
    and `load(libs)`; each version module holds `HW: OnceLock<Result<HwApi, String>>`, set in
    `rescan_with` after the required `Api`. t961 green; t10/t21/t37/t849 green (workspace run).
- [x] 2.3 Commit stage 2
  - Committed with stages 1, 3, 4.

## 3. The VAAPI path (D2–D4, D7)

- [x] 3.1 `encoders.rs`: `HwKind` on every candidate, `h264_vaapi` = `Vaapi`/`Nv12`, its bitrate options. Verify t858 and t960 green
  - `HwKind { None, Vaapi }` on every candidate; `h264_vaapi` = `Vaapi`/`Nv12`, `frames_size`,
    bitrate mode `rc_mode=VBR` (D7). t858, t960 green.
- [x] 3.2 `muxer_impl.rs`: the device → frames → open step with skip reasons kept for the open report; the per-frame upload with pts and duration; the `Drop` order. Verify t962 green, t25/t33 green, and the golden stream hashes (t11, t485) unchanged
  - `Muxer::open`: device (`hw_seam`/`KINETREN_VAAPI_DEVICE` alone, else `renderD128…135` that
    exist) → frames context (VAAPI/NV12, export size, pool 20) → encoder with `hw_frames_ctx`;
    every part that fails frees what it made and lands in `OpenReport.skipped` (or `tried` for the
    encoder's code), `report.device` names the node. `write_video` uploads the NV12 frame to a
    surface (pts; `encode` sets the duration on the surface frame), unrefs it after the send;
    `Drop` frees frames → contexts → frames ctx → device. `ExportLog::Skipped` reaches the window
    log and kt-cli. t962, t25, t33, t954, t955 green; GOLDEN untouched (no software path change).
- [ ] 3.3 On the owner's machine: `KT_EXPECT_HW=vaapi make hw-test` (new Makefile target running the FFmpeg tests on the host with the variable). Verify t963 green; record the device, driver vendor string, FFmpeg major and the measured mean difference, and set the bound in D8
  - Needs the owner's machine (a VAAPI render node): `KT_EXPECT_HW=vaapi make hw-test` (new
    target: t963 on the host, then `kt-cli ffmpeg --probe-encoders`). The bound in D8 is still the
    proposal's 6/255.
- [x] 3.4 Commit stage 3
  - Committed with stages 1, 2, 4.

## 4. The list, the report, the window (D6)

- [x] 4.1 `available_encoders()` with the device; `hw_report()`; `kt-cli ffmpeg` section and `--probe-encoders`. Verify t964 and t849 green
  - `available_encoders()` → `AvailableEncoder { name, device }` (a device candidate listed only when
    its DEVICE opens — never the encoder); `hw_report()` (the table, each render node with the
    libva vendor string via `vaQueryVendorString` on the device's display); `kt-cli ffmpeg` prints
    the section, `--probe-encoders` opens each hardware candidate with the export's own `open` at
    1280×720 into a throwaway file and prints "opens [on <device>]" or "fails: <part>". Here:
    "render nodes: none in /dev/dri", "h264_vaapi: fails: no VAAPI device (no render node in
    /dev/dri)". t964, t849 green.
- [x] 4.2 `render_dialog.rs`: the combo from the new list with the device; the progress row names a hardware encoder's device (needs `render-window-v1-parity`'s encoder row). Verify t965 green and the ui-reach/ui-timeline guards (`reach_tests.rs`, `layout_tests.rs`) green
  - The combo reads `render_setup.encoders`, asked once in `start_render` (the device open is not a
    per-frame cost), "h264_vaapi (/dev/dri/renderD128)"; the progress row "h264_vaapi (hardware ·
    /dev/dri/renderD128) · nv12 · QP 23". One new catalog key (`render.log_skipped`, five languages).
    t965 green; kt-app guards green in the workspace run.
- [ ] 4.3 Speed on release: `cargo build --release -p kt-cli`; a 1080p 60 s export with `libx264` and with `h264_vaapi` on the owner's machine; record both wall times here (release only)
  - Needs the owner's machine (release build, a 1080p 60 s export with libx264 and h264_vaapi).
- [ ] 4.4 `openspec/specs/export/design.md`: remove the "known remainder" about vaapi/qsv never opening from "Non-obvious details" and the matching "Deferred" item, and the media-index "Deferred" line (at archive). Verify `openspec validate --specs` is green
- [ ] 4.5 Commit stage 4 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-ffmpeg -p kt-engine -p kt-cli -p kt-app --all-targets` with no new warnings)

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` here, with the t963 record of 3.3
- [ ] 5.2 After archiving: set F-EXP-004's verdict line to the delivered state (`CAPABILITY:` unchanged), and append the "why" (D1 the probe's finding, D3 skip-before-open, D5 optional table, D8 loud opt-in proof) to `openspec/specs/export/design.md` and `openspec/specs/media-index/design.md`
