## MODIFIED Requirements

### Requirement: Export is fully in-process, the encoder is chosen by opening it
Muxing and encoding SHALL go through libavformat/libavcodec of the FFmpeg installed on the machine
and loaded at runtime (capability `media-index`): no temporary files, no external
`ffmpeg` binary, no mkfifo. The video encoder SHALL be chosen by trying candidates with a REAL
open check (`avcodec_open2`) in the same process that writes the file.
The list of candidates depends on the OS: on Linux
`h264_nvenc → h264_vaapi → h264_qsv → libx264 → mpeg4`, on Windows
`h264_nvenc → h264_amf → h264_qsv → libx264 → h264_mf → mpeg4`. `h264_qsv` SHALL
be opened with the pixel format NV12 (swscale conversion), not YUV420P. Each candidate
SHALL get its own quality setting (CRF/CQ/QP/global quality — depending on the encoder kind).

A candidate that needs a hardware device SHALL be tried as ONE step made of three parts — open the
device, create its frames context, open the encoder with that context — and SHALL count as opened
only when all three succeed; any part failing frees what was created and moves on to the next
candidate. `h264_vaapi` SHALL be opened on a VAAPI device (the render node named by
`KINETREN_VAAPI_DEVICE`, otherwise the first of `/dev/dri/renderD128` … `renderD135` that opens as a
VAAPI device) with a frames context of VAAPI surfaces whose software format is NV12 and whose size
is the export size; every frame SHALL be converted to NV12 and uploaded to a surface of that
context before it is sent, keeping its pts and duration. When no device opens, or the hardware
context functions are missing from the loaded FFmpeg (capability `media-index`), the candidate
SHALL be skipped WITHOUT calling `avcodec_open2`, and the reason SHALL be kept for the export's
report.

An explicitly requested encoder MUST either open, or give an export error — there is no
silent substitution; for a hardware encoder the error SHALL name the failing part (no device,
the frames context refused, or the encoder's code) and MUST NOT end the process. The name of the
actual encoder SHALL be reported in the export statistics. Frames MUST NOT
be lost on encoder backpressure: on EAGAIN the send is repeated.
Why: the presence of an encoder in the build does not mean it will work (nvenc without libcuda crashes),
and we have no cached "verdict of fitness", so there is nothing that can diverge from reality.
On Windows there is no VAAPI; AMD and Intel without their own encoders in the list fell back to `mpeg4`
if the FFmpeg build has no libx264 (LGPL builds). `h264_mf` (Media Foundation) is present on any
Windows, but its quality is worse than libx264's, so it comes after libx264. QSV did not
open with YUV420P at all. The VAAPI encoder accepts only VAAPI surfaces: without a device and a
frames context it refused every open (−22), so Intel and AMD on Linux always fell back to libx264,
and a forced `h264_vaapi` ended the program at render (ktprompts v26.10 T16). Skipping before
`avcodec_open2` keeps the encoder's failed-init path out of the cascade altogether.

#### Scenario: Auto selection and forced encoder
- **WHEN** the export starts without an encoder given / with `libx264` / with a non-existent name
- **THEN** auto picks the candidate that opened; `libx264` is chosen exactly, and the file is read by its own decoder; an unknown name is an explicit error, no file is created
- **TEST** t25 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: All frames in the file
- **WHEN** 60 frames are exported at 30 fps
- **THEN** the engine delivered 60 frames AND in the file, read by its own decoder over all keys of the grid, no fewer than 60 are decoded
- **TEST** t33 (`src/libs/kt-engine/tests/export_options.rs`), t10 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`) — our own decoder is fit as a verifier because its seek is sample-accurate

#### Scenario: Cancel and error leave no partial file
- **WHEN** the export is cancelled or crashes in the middle of the video/audio phase
- **THEN** the muxer is closed, the partial file is deleted, the result is `Cancelled` or an error
- **TEST** t25 family (`src/libs/kt-engine/tests/export_options.rs`) — a forced encoder error leaves no file; the cancel path is `export_mp4_with_backends` (`src/libs/kt-engine/src/export.rs`)

#### Scenario: The candidate list and its settings per OS
- **WHEN** the candidate list and the opening settings are built for Linux and for Windows (a pure function, checked on any OS)
- **THEN** the order matches the requirement for each OS; `h264_vaapi` is only in the Linux list, `h264_amf` and `h264_mf` — only in the Windows list; `h264_qsv` has the pixel format NV12; each candidate has a quality setting
- **TEST** t858 (`src/libs/kt-ffmpeg/tests/encoders.rs`)

#### Scenario: Each candidate declares the context it needs
- **WHEN** the candidate lists of both OSes are built (pure, any OS)
- **THEN** `h264_vaapi` declares a VAAPI device with an NV12 upload and its frames-context size
  equal to the export size; `h264_nvenc`, `h264_amf`, `h264_mf`, `libx264` and `mpeg4` declare
  none; `h264_qsv` declares what the investigation stage recorded in `design.md` (system NV12
  frames unless it is changed there); a device candidate's quality options are those of t858
- **TEST** t960 (`src/libs/kt-ffmpeg/tests/encoders.rs`)

#### Scenario: Without a device the cascade falls through deterministically and a forced VAAPI is an error, not a crash
- **WHEN** `KINETREN_VAAPI_DEVICE` points at a path that does not exist and a 30-frame export is run
  twice with the encoder left to the cascade; then once with `h264_vaapi` forced; then once with
  `h264_vaapi` forced and the hardware context functions reported missing by the test seam
- **THEN** both cascade runs choose the same encoder, and if the cascade reached `h264_vaapi` its
  report entry says "no VAAPI device" and the encoder's open was never called for it; the forced
  runs return an export error naming "no VAAPI device" and "hardware context functions missing"
  respectively, leave no output file, and the test process is still running afterwards
- **TEST** t962 (`src/libs/kt-engine/tests/export_options.rs`)

## ADDED Requirements

### Requirement: A real VAAPI export produces a file that decodes, proven on hardware
On a machine with a VAAPI render node whose driver encodes H.264, an export with `h264_vaapi`
forced SHALL produce an MP4 in which our own decoder reads every exported frame, and the export
statistics and the encoder report SHALL name `h264_vaapi`, hardware, and the device used. This
check SHALL run when `KT_EXPECT_HW=vaapi` is set and SHALL fail the run when VAAPI does not open
there (a skip is not a pass); without the variable it SHALL be skipped with the reason printed.
CI runners have no GPU, so the check is run by `make hw-test` on the owner's machine and recorded in
the change's `tasks.md`.
Why: a cascade test on a machine without a GPU proves only the fall-through; the encode path
itself (surfaces, upload, flush) is reachable only on hardware, and "it ran green" with the check
silently skipped would prove nothing — the same rule as `KT_EXPECT_FFMPEG_MAJOR` (t865).

#### Scenario: Sixty frames through VAAPI
- **WHEN** with `KT_EXPECT_HW=vaapi` a 60-frame 1280×720 export at 30 fps of the synthetic ramp
  project is run with `h264_vaapi` forced
- **THEN** the file holds an H.264 stream from which our decoder reads 60 frames over all keys of
  the grid, the mean absolute difference of each decoded frame from the raw export frame is under
  the bound recorded in `design.md`, and the statistics name `h264_vaapi` with the device path;
  with `KT_EXPECT_HW=vaapi` on a machine without a usable node the test fails naming the reason
- **TEST** t963 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: The render settings offer a hardware encoder only when it can be opened here
The encoder list of the render settings window SHALL contain `h264_vaapi` only when the loaded
FFmpeg has the encoder, its hardware context functions are present and a render node opens as a
VAAPI device; the entry SHALL show the device it will use. Other candidates keep the existing rule
(present in the build). The progress window's video row (change `render-window-v1-parity`) SHALL
name a hardware encoder together with its device.
Why: F-EXP-004 — "the list shown to the user must coincide with what will actually open"; today
`h264_vaapi` and `h264_qsv` are offered and never open. Opening the device is cheap; opening the
encoder is the export's own check and is not repeated in the window.

#### Scenario: The combo and the progress line follow the device
- **WHEN** the harness opens Render settings with the encoder list reported (a) with a VAAPI
  device `/dev/dri/renderD128` and (b) without one; then a scripted job reports the opened encoder
  `h264_vaapi` on `/dev/dri/renderD128`
- **THEN** in (a) the combo offers "h264_vaapi (/dev/dri/renderD128)", in (b) it does not offer
  `h264_vaapi` at all; the progress window's video row reads `h264_vaapi`, "hardware" and the
  device path
- **PATH** Render… → Render settings → open the "Video codec" list → "Auto (hardware first)",
  "h264_nvenc", "h264_vaapi (/dev/dri/renderD128)", "libx264" → choose h264_vaapi → Render… → file
  name → the progress window: "Video: h264_vaapi (hardware · /dev/dri/renderD128) · nv12 · QP 23"
- **TEST** t965 (`src/apps/kt-app/src/layout_tests.rs`)
