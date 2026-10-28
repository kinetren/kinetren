## ADDED Requirements

### Requirement: Hardware-context functions are optional symbols
The FFmpeg functions needed only for hardware encoding contexts (`av_hwdevice_ctx_create`,
`av_hwframe_ctx_alloc`, `av_hwframe_ctx_init`, `av_hwframe_get_buffer`,
`av_hwframe_transfer_data`, `av_buffer_ref`, `av_buffer_unref`) SHALL be resolved in a table of
their own after the required table has loaded. If any of them is missing, loading SHALL still
succeed with the hardware table absent, the search report SHALL say which function is missing,
and every encoder candidate that needs a device context SHALL be skipped (capability `export`).
The struct layouts these functions use (`AVHWFramesContext`, the device and pixel-format enums)
SHALL come from the version module of the loaded major, generated from the vendored
`libavutil/hwcontext.h` of each supported major, like every other type.
Why: invariant 10 promises one binary for every FFmpeg the machine has; a trimmed or custom build
without `hwcontext` must keep opening media. Today every symbol in `api.rs` is required, so one
more required name would turn a working machine into "FFmpeg not found".

#### Scenario: A missing hardware function does not fail the load
- **WHEN** the hardware table is loaded through a resolver that knows every name, then through
  one that lacks `av_hwframe_transfer_data`; and for each vendored major (6, 7, 8, 9) the version
  module is asked for the VAAPI device type, the VAAPI and NV12 pixel formats and the size of
  `AVHWFramesContext`
- **THEN** the first gives a hardware table; the second gives none and the reason names
  `av_hwframe_transfer_data`, while the required table is unaffected; every major's module has the
  types, and the enum values equal those of its vendored `hwcontext.h`/`pixfmt.h`
- **TEST** t961 (`src/libs/kt-ffmpeg/tests/encoders.rs`)

## MODIFIED Requirements

### Requirement: The FFmpeg search report is visible to the person and to kt-cli
The FFmpeg search result SHALL be available as a report: the chosen directory (or the SONAME on
Linux), the major and the full version of avcodec, and each rejected candidate with a reason. The GUI
shows it in the FFmpeg window; `kt-cli ffmpeg` prints it and exits with a non-zero code
if FFmpeg is not loaded. The error text ""FFmpeg not found"" MUST hint at the way of its OS:
on Linux — the distribution's packages, on Windows — the download button or the installation of a shared build
of a supported major.

When FFmpeg is loaded, the report SHALL also carry a "hardware encoding" section: whether the
hardware context functions are present (or which one is missing), and on Linux each render node
`/dev/dri/renderD128` … `renderD135` that exists, with whether it opens as a VAAPI device and, when it
does, the driver's vendor string. `kt-cli ffmpeg --probe-encoders` SHALL additionally try to open
every hardware candidate of the OS cascade with the export's own opening code at 1280×720 and
print, per candidate, "opens" or the failing part and its code; without the flag nothing is
opened beyond the devices. The section never changes the exit code.
Why: on Windows there are several causes of failure, and they are not obvious (a static build, FFmpeg 9,
a mixed set). Without a named cause the user sees only placeholders and does not know
what to do. The same holds for hardware encoding: "no hardware encoder" has a device cause (no
render node in the sandbox, no driver, a driver that cannot encode H.264) that only the machine
itself can name, and the Flatpak guide sends people to this report.

#### Scenario: kt-cli prints the report
- **WHEN** `kt-cli ffmpeg` is started with `KINETREN_FFMPEG_DIR` pointing at an empty directory, and with FFmpeg available by the usual path; then without an available FFmpeg
- **THEN** in the first case the output names the empty directory as rejected ("no avcodec-*.dll"), the chosen candidate and its version, exit code 0; in the second — the list of rejected ones and a hint for its OS, a non-zero exit code
- **TEST** t849 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: kt-cli names the hardware encoding devices
- **WHEN** `kt-cli ffmpeg` runs with FFmpeg loaded and `KT_DRI_DIR` (the test seam for `/dev/dri`)
  pointing at a temp directory holding a regular file `renderD128`; then with `--probe-encoders`
- **THEN** the output has a "hardware encoding" section saying the hardware context functions are
  present (or naming the missing one), listing `renderD128` as "does not open as a VAAPI device"
  with a reason, and the exit code is 0; with `--probe-encoders` there is one line per hardware
  candidate of the OS cascade, each "opens" or naming the failing part, and the run ends normally
- **TEST** t964 (`src/apps/kt-cli/tests/cli.rs`)
