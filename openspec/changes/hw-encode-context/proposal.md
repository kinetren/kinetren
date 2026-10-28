# Proposal

## Why

On Linux only NVENC can encode in hardware: `h264_vaapi` is offered in the render window and is in
the cascade, but it never opens — the muxer gives it no device and no frames context, and the
VAAPI encoder refuses a context without `hw_frames_ctx` (−22, EINVAL, measured for F-EXP-004;
`openspec/specs/export/design.md`, "Non-obvious details" and "Deferred"). So Intel and AMD
machines always fall back to libx264, and choosing `h264_vaapi` by hand "just crashes the
application at render" (ktprompts v26.10 T16; R.71 K6 assigns it here). The owner chose a Flathub
release with a VAAPI context (2026-10-27, `docs/user-feedback.md`): `flatpak-flathub` depends on
this change — "video acceleration must work in render".

## What Changes

- **A VAAPI encode path on Linux**: before `h264_vaapi` is opened, the muxer opens a VAAPI
  device (the render node named by `KINETREN_VAAPI_DEVICE`, otherwise `/dev/dri/renderD128…135`
  in order), creates a frames context (`VAAPI` surfaces, `NV12` software format, the export size),
  and opens the encoder with it. Each frame is converted RGBA → NV12 by swscale as today and
  uploaded to a surface (`av_hwframe_get_buffer` + `av_hwframe_transfer_data`) before it is sent.
- **The real-open cascade stays** (invariant 9): a hardware candidate is "device + frames context
  + `avcodec_open2` all succeeded"; any step failing moves to the next candidate. A candidate whose
  device cannot be opened is skipped BEFORE `avcodec_open2` is called, with the reason kept for the
  report.
- **T16**: a forced `h264_vaapi` without a usable device ends in an explicit export error naming
  the reason ("no VAAPI device", "VAAPI device opened but the encoder refused: code N"); no crash,
  no file left. The first stage proves where the crash came from before code is changed.
- **QSV and Windows**: `h264_qsv` keeps NV12 system-memory frames (it already gets NV12 since
  windows-build); whether it needs a QSV/D3D11 device context on either OS is decided by the
  investigation stage, not assumed. AMF and Media Foundation keep system-memory frames.
- **New FFmpeg symbols are optional** (invariant 10): the hw-context functions are resolved in a
  separate table; if any is missing, FFmpeg still loads and the candidates that need a context are
  skipped with "hardware context functions missing". The bindgen types gain `libavutil/hwcontext.h`
  for every vendored major (6/7/8/9).
- **The list matches what can open**: `available_encoders()` offers `h264_vaapi` only when the
  hw functions are present and a render node opens as a VAAPI device; the render settings combo
  and `kt-cli ffmpeg` show it with its device. `kt-cli ffmpeg` gains a "hardware encoding" section
  (devices found, and an opt-in open probe of each hardware candidate) — the tool the Flatpak
  guide will tell people to run inside the sandbox.
- The encoder line of the render window (change `render-window-v1-parity`) names the hardware
  encoder and its device.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `export`: the encoder-cascade requirement gains the device + frames context for VAAPI, the
  skip-before-open rule for a missing device, and the explicit error for a forced hardware encoder
  that cannot open; a gated requirement that a real VAAPI export decodes.
- `media-index`: the FFmpeg loading requirement gains optional symbols — the hardware-context
  functions are looked up separately and their absence never fails the load; the search report
  (`kt-cli ffmpeg`) names the hardware encoding devices.

## Impact

- Code: `src/libs/kt-ffmpeg/build.rs` (add `libavutil/hwcontext.h` to the bound headers),
  `src/libs/kt-ffmpeg/src/api.rs` (an optional `HwApi` table), `src/libs/kt-ffmpeg/src/lib.rs`
  (dispatch, `available_encoders`, a `hw_report()`), `src/libs/kt-ffmpeg/src/encoders.rs` (each
  candidate's context kind), `src/libs/kt-ffmpeg/src/muxer_impl.rs` (device/frames context,
  upload, cleanup order), `src/apps/kt-cli/src/main.rs` (`kt-cli ffmpeg` hardware section),
  `src/apps/kt-app/src/render_dialog.rs` (the combo from the new list).
- Tests: `src/libs/kt-ffmpeg/tests/encoders.rs`, `src/libs/kt-engine/tests/export_options.rs`,
  `src/apps/kt-cli/tests/cli.rs`, `src/apps/kt-app/src/layout_tests.rs`; a hardware-gated test
  run by `KT_EXPECT_HW=vaapi` on a machine with a render node (not on CI runners).
- No new crate, no project-format change, the raw-stream golden references are untouched
  (they hash streams BEFORE the muxer). Invariants 9, 10 and 11 are kept, not changed.
- Feature note F-EXP-004 (`docs/features/export/pipeline/hardware-encoder-selection.md`) gets a
  dated revision of its verdict.
