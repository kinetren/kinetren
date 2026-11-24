# Export and headless render

## Purpose

Offline run of the same engine as the preview: in-process muxing to mp4 through the system
FFmpeg, golden references for raw streams, the headless `kt-cli` tools (render/export/probe),
and cross-render against another editor as external conformance.

## Requirements

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

A candidate whose open loads a libva driver into the process (`h264_vaapi`, and `h264_qsv` on Linux)
SHALL, when a render node exists, first be opened by a throwaway process of the same binary, which
encodes a few frames into `/dev/null` and keeps nothing. Only its success SHALL let the driver into
the render process; a signal, its time limit or a refusal SHALL skip the candidate with the reason,
as above. Each export asks again. This process is the one exception to "in-process": it is our own
binary, it writes no file, and the export itself stays in one process.

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
`avcodec_open2` keeps the encoder's failed-init path out of the cascade altogether. A driver that
faults gives no return code to fall down on, so it can only be met outside the process that holds the
render: until this change Auto on Intel and AMD never touched a driver, and without the throwaway
process it would have started to risk every render there.

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
- **WHEN** the device (`KINETREN_VAAPI_DEVICE`, in the test its thread-local seam) points at a path
  that does not exist and a 30-frame export is run
  twice with the encoder left to the cascade; then once with `h264_vaapi` forced; then once with
  `h264_vaapi` forced and the hardware context functions reported missing by the test seam
- **THEN** both cascade runs choose the same encoder, and if the cascade reached `h264_vaapi` its
  report entry says "no VAAPI device" and the encoder's open was never called for it; the forced
  runs return an export error naming "no VAAPI device" and "hardware context functions missing"
  respectively, leave no output file, and the test process is still running afterwards
- **TEST** t962 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A driver that crashes does not end the render
- **WHEN** with the fake driver a render node is present and the driver aborts on load, in the
  encoder's open and on the first picture; a render is run with `h264_vaapi` forced, then left to
  the cascade; then `kt-cli ffmpeg --probe-encoders`
- **THEN** forced, the process ends by itself with an error naming "the driver crashed in a test
  process (signal 6)" and leaves no file; left to the cascade the render finishes on the next
  encoder with the skip in its log; the report finishes and names the crash, also for the device
- **TEST** t1551 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Each refused part is named and the cascade goes on
- **WHEN** the fake driver refuses, in turn, to load, to create surfaces, to create the encoder's
  context, to encode the first picture, and encodes it into nothing
- **THEN** forced, the error names "no VAAPI device", "frames context: code", "encoder: code", "first
  frames: encoder send" and "first frames: the encoder produced no data", no file left; left to the
  cascade the render finishes on the next encoder and its log names the same part
- **TEST** t1552 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A driver that hangs is left behind
- **WHEN** the fake driver never returns from the encoder's open and a render is left to the cascade
- **THEN** the throwaway process is killed at its time limit, the log says "the driver did not
  answer", the render finishes on the next encoder
- **TEST** t1553 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A hardware failure in the middle of a render is an error with no partial file
- **WHEN** the fake driver passes the trial and refuses the ninth picture of a forced `h264_vaapi`
- **THEN** the render starts on `h264_vaapi` and ends with an error naming the encoder's send; the
  process lives and no file is left
- **TEST** t1554 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A healthy driver gets every frame, after the trial
- **WHEN** a 30-frame render is left to the cascade on the healthy fake driver with no NVENC here
- **THEN** `h264_vaapi` opens, hardware, NV12; the driver saw three pictures in the throwaway process
  and all 30 in the render's own
- **TEST** t1555 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The candidates that load a VA driver are tried apart
- **WHEN** the candidate lists of both OSes are built (pure, any OS)
- **THEN** on Linux `h264_vaapi` and `h264_qsv` declare the trial, in that order; nothing else, and
  nothing on Windows
- **TEST** t1550 (`src/libs/kt-ffmpeg/tests/encoders.rs`)

#### Scenario: A hardware encoder that cannot work, through the window
- **WHEN** the real program runs on a driver that crashes in the encoder's open, `h264_vaapi` is
  chosen in the render settings and rendered; then libx264 is; then on a healthy driver Auto is
- **THEN** the first render ends in the window with the error and Open log, no file, the editor
  alive; the second finishes; the third finishes on the hardware encoder and the file is an MP4
- **PATH** File → Render… → the "Video codec" list offers "h264_vaapi (…/renderD128)" → choose it
  → Render… → the window: the error with the reason, Open log, Close → Close → the editor is
  there → File → Render… → choose libx264 → Render… → Replace if asked → the summary line, Open file
- **TEST** live scenario `hwencode` (`src/scripts/live/hwencode.py`)

### Requirement: Preview == export byte-for-byte on one backend
Export SHALL use the same frame plan and the same audio/video renderers as the preview
(capability `video-plan`, invariant 7): the only differences are the "clock by readiness" and
Quality = project resolution. An export stream frame MUST be byte-for-byte equal to the
preview composite of the same moment at the same Quality — with an empty chain, with an FX chain
(cold, and warm from the executor's memo), with a generator, and with a title; on GPU, by the same
GPU executor. Source facts (native, procedural) export SHALL query open sources the same way as the
player, and the list of generator sources SHALL be computed by ONE code path (`gen_sources`) for the player, export and kt-cli.
Why: two composition codes ("PlaybackPipeline versus ProjectRenderer") were a defect n
of the old engine: the preview showed one thing, and the file contained another.

#### Scenario: Export frame equals preview
- **WHEN** frame No. 17 of the export stream and the preview composite of the same tick at the same Quality are taken
- **THEN** the bytes are equal
- **TEST** t11_preview_equals_export_frame (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: With FX chain, cold and warm
- **WHEN** on video clips the chain is Mirror → Glow → VHS; the preview is composited twice (the second time from the readback memo)
- **THEN** both composites are byte-for-byte equal to the export frame
- **TEST** t67 (`src/libs/kt-engine/tests/golden_export.rs`) — the export composite does NOT memoize, but the bytes are the same

#### Scenario: Generator and title in the render path
- **WHEN** the scene contains a solid generator / a title (capability `generators`)
- **THEN** export is deterministic between runs, the frame in the generator's zone has the set color, and preview == export byte-for-byte
- **TEST** t479 (`src/libs/kt-engine/tests/golden_export.rs`), t485_title_preview_equals_export (`src/libs/kt-engine/tests/title_golden.rs`)

#### Scenario: On GPU, by the same executor
- **WHEN** an adapter is present; preview and export are executed by the GPU backend, cold and warm
- **THEN** frames are equal byte-for-byte (without an adapter — skip; `KT_REQUIRE_GPU=1` requires it)
- **TEST** t184 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: An export needs a device; a device's error stops it
The GPU executor SHALL be passed explicitly by the product (kt-app/kt-cli), and the render window SHALL
say when there is none (`BackendLine::None`). The refusal with `ExportError::NoDevice` and a GPU error as
an export error are capability `video-plan`'s rule ("The GPU is the one executor"); this requirement keeps
the export's scenarios.
Why: the CPU composite is deleted (compute-kernels-render, owner, 2026-11-04), so no second way to
compose a frame exists, and an export that writes no picture must not look like one that did.

#### Scenario: An export carries 3D, and with no device it refuses
- **WHEN** a project with Pan3d is rendered on a device; then a project is rendered with `ExportBackends::default()` (no executor)
- **THEN** the export passes and the frame differs from the frame without the effect; the second render fails with "no video device"
- **TEST** t301 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: No device is a refusal before anything is written
- **WHEN** a project is rendered with no executor among its backends
- **THEN** the render fails before it writes anything, and the error reads "no video device"
- **TEST** t1575 (`src/libs/kt-gpu/tests/gpu_render.rs`, `t301_export_renders_3d_and_t1575_no_device_is_a_refusal`)

### Requirement: Golden references for raw streams
The raw stream functions (`render_raw_audio`/`render_raw_video`) SHALL deliver audio f32
48 kHz/2ch and RGBA frames to the callbacks BEFORE the muxer; their hashes MUST be byte-for-byte equal between
runs and to the committed `GOLDEN_AUDIO`/`GOLDEN_VIDEO` of EVERY
shipped platform (a table by `(os, arch)`: linux/x86_64, windows/x86_64); the title
scene — to its own `GOLDEN_TITLE_VIDEO` from the same table. Hash checks not tied to a platform (video FX, FX thumbnails, merging of geometry series) keep one
constant for all platforms. On a platform that is not in the table,
the comparison with the constant is skipped with a named reason, while the check between runs
remains. A video hash SHALL be compared on a software adapter only (`hash_holds`): a card is held to
the stored frames within tolerances (capability `video-plan`). The reference changes ONLY by a conscious decision with a record of the reason and the previous value
next to the constant (for the title — also in `FONT-MANIFEST.txt`); in the reference projects there is no net
zoom and the native synthetic size is less than Quality, so the decode scale step there is ≡ 1, and new
mechanisms MUST leave the reference in place. The constant of a new platform is entered after the
first green run on it, with a record of which machine and which run it was taken on.
Why: the reference is the single guard of determinism (invariant 11) at once for the audio mix,
warp, transitions and the composite; the bytes of the output .mp4 will never be
identical for two users (the encoder is a property of the machine), so the stream is hashed BEFORE the
encoder. The table by platform was made while transcendental functions came from the platform's
libm (glibc, the mingw CRT) and could diverge in the last bit; since t1066 the frozen paths use
`kt_plugin_api::math`, so both rows hold the same constants and the table stays as the place a new
platform is admitted.

#### Scenario: Determinism and the reference
- **WHEN** the golden project is rendered twice
- **THEN** the audio and video hashes match between runs (120,000 audio frames, 75 video frames) and equal the committed constants of its platform (linux/x86_64, windows/x86_64)
- **TEST** t11_streams_are_deterministic_across_runs, t11_streams_match_committed_golden (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: The title reference is separate and does not move
- **WHEN** the title scene is rendered twice / the title parameters have moved onto the event
- **THEN** the hash is stable and equals `GOLDEN_TITLE_VIDEO` of its platform; moving the values did not shift the reference
- **TEST** t485_title_export_is_deterministic, t485_title_matches_committed_golden, t662 (`src/libs/kt-engine/tests/title_golden.rs`)

#### Scenario: Each shipped platform has its own constants
- **WHEN** the export and title reference tables are read by the guard (the video-FX, FX-thumbnail and geometry-merging hashes are not tied to a platform and matched on windows-latest, so they keep one constant for all platforms)
- **THEN** each platform from the shipped list (linux/x86_64, windows/x86_64) has an entry in each table, and each entry has a provenance line (machine, run); on a platform without an entry the comparison test is skipped with a named reason, rather than passing silently
- **TEST** t859 (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: Export chunk is computed from memory, layers and decode quality
The SERIAL export (the reference export, `ExportOptions.serial` / `kt-cli export --serial`; a stream stage takes the pipeline, "A stream stage takes the pipelined export") SHALL decode frames in batches via span reads (by the same path as the preview) and
composite from the cache; the batch size MUST be computed from the cache budget (no more than half),
the frame size at the LARGEST decode quality (decode scale step S gives S² memory), and the number
of simultaneous video layers (a blending layer counts as two frames; a pair inside a reel is counted).
The pipelined export counts by what its frames ask and keeps them by holds (requirements on the
pipeline below). The finished export frame MUST NOT
depend on cache timing: the batch must survive until composition.
Why: "black render" (120 frames of 1080p = 995 MB did not fit into the cache, and the LRU evicted them before
composition) and "broken render" (a batch for one layer displaced the pre-frames of the neighbouring track;
post-FX frames of 8.3 MB displaced pre-frames not yet used) — both regressions were seen
by the owner, and both showed up only at a large Quality.

#### Scenario: A batch does not exceed half the budget, accounting for layers
- **WHEN** the chunk size is computed for a large Quality and N layers
- **THEN** the batch ≤ half the budget / N, and live export at large Quality yields no black frames
- **TEST** t116 (`src/libs/kt-engine/tests/frame_order.rs`)

#### Scenario: A decode scale step shrinks the chunk
- **WHEN** a clip is taken at a ×2 step (native with headroom, zoom ×2)
- **THEN** the chunk by decode quality is four times smaller than the chunk by Quality, and the live render has `missing_video_layers == 0`
- **TEST** t314 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A blending pair and nesting are counted in the budget
- **WHEN** inside a reel frames are blended as a pair (capability `compositions`)
- **THEN** the chunk budget counts both frames of the pair, starvation does not trigger, and the bytes do not depend on cache timing
- **TEST** t583 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Span reading equals per-frame reading
- **WHEN** export reads a batch of keys as a span
- **THEN** the frames are byte-for-byte equal to per-frame reading, and the decoder is not reopened
- **TEST** t21 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

### Requirement: Render range — up to the edit length; the region is GUI-only
The render length SHALL be `max(end of the material, project timeline_end)`: a trailing Gap
of someone else's file is rendered as black/silence. The GUI SHALL offer export of the selected region;
kt-cli ignores the region — by a direct decision of the owner ("we do not do it").
Why: foreign files set the editing length with emptiness after the last clip, and without it
our render was shorter than melt by 605 frames (found by cross-render).

#### Scenario: The trailing Gap is rendered
- **WHEN** a foreign .otio with a Gap after the last clip is rendered by us and by melt
- **THEN** the durations match
- **TEST** t158 (`src/apps/kt-cli/tests/cross_render.rs`)

### Requirement: Loudness is measured in the export pass, without master gain
Export SHALL report the integrated loudness, the true peak and the LRA (BS.1770/EBU R128)
of the finished mix, measured as capability `audio-render` states it ("Loudness — BS.1770 / EBU R 128",
the rule's home: the finished buffer, no extra pass, no master gain). The master gain MUST NOT exist in the
project model: it is a monitoring loudness that lives only in the player. There is no "normalize" button —
a reference line "to −23 LUFS: +N dB" is shown.
Why: the number in the preview and the number in the file match BY CONSTRUCTION only if one
code counts both over the same material; normalizing after the fact would mean re-encoding the file.

#### Scenario: Loudness hears the event level
- **WHEN** the same event is exported at 0 dB and at −10 dB
- **THEN** the difference is exactly 10 LU (±0.15), and the true peak matches the amplitude of the source
- **TEST** t406 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The master gain is not in the measurement
- **WHEN** the project settings are printed and the export is repeated
- **THEN** there is no master-gain field in the project, and the number is deterministic
- **TEST** t407 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The meter that export measures with
- **WHEN** the meter receives a calibration sine / an inter-sample overshoot / the same signal cut differently
- **THEN** 1 kHz at full scale = 0.0 LUFS; the true peak sees the overshoot between samples; the cutting does not change the result; the sums are in f64, because f32 accumulation over 10 minutes drifts by 0.37 LU (a guard of the APPROVAL, not of the code)
- **TEST** t403, t403b, t405 (`src/libs/kt-fx/tests/loudness.rs`) — the meter itself belongs to capability `audio-render`

### Requirement: A render with unavailable sources proceeds rather than being cancelled
In the GUI an unavailable source SHALL get a stub (a dark plate with the file name; audio is
silence); the settings window BEFORE the start SHALL list such sources, and the summary AFTER SHALL
show the number of stubs and the engine's `missing_video_layers`. In kt-cli unavailable media is
a REFUSAL without an output file; `--allow-missing` switches this on deliberately.
Why: the owner's requirement — "missing sources must not be a blocker: notify and
render"; but kt-cli has no dialog, and silence in a file is worse than a refusal.

#### Scenario: The GUI warns and renders
- **WHEN** the project has `gone.mp4`, and the render window opens the sources
- **THEN** the export is not cancelled, the list of unavailable items is visible before the start, and the lost layers are in the summary
- **TEST** t210 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: kt-cli refuses without the flag
- **WHEN** `kt-cli render` receives a project with nonexistent files
- **THEN** an error code is returned and there is no file; with `--allow-missing` the render passes
- **TEST** t389 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: Render file name — by the project, region and length template
The default name SHALL be `kt-<project>-<region>-<length>s.<ext>` (the extension follows the container), where the region is the name of the
range marker MATCHING the region (no match or an empty name — the segment is omitted),
the length is the whole seconds of the range; spaces → `-`, forbidden file-system characters → `_`.
Why: the owner's requirement verbatim; the former `kp-render-<unix-time>` said nothing about
the project or the region.

#### Scenario: Template and a live region
- **WHEN** the project `song.otio`, the marker `chorus` 1…5 s, and the region 1…5 s / the region 0…5 s without a marker
- **THEN** `kt-song-chorus-4s.mp4` / `kt-song-5s.mp4`; name sanitizing is checked on "scene 2: take?"
- **TEST** t201 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: kt-cli — the headless interface of the same engine
`kt-cli` SHALL be able to `new`, `demo`, `roundtrip`, `convert`, `probe`, `render` (wav),
`export` (the container by the file's extension, the third argument is a forced encoder). Writing a project SHALL go through the same
atomic writer as the GUI; an existing file MUST NOT be overwritten without `--force`.
`demo` by default SHALL refer to synthetic sources (`synth:`), so that
`demo && render` works without a single media file. Relative media paths SHALL be resolved
exactly once (when the project is loaded), and `.ktproj` in `render/export` — a refusal with a hint to
`convert`. Export opens the files with the project's media index.
Why: kt-cli has neither an asterisk nor a dialog — there was nothing to prevent silent overwriting;
`demo` referred to eight files that nobody had; a double join of the path base silently
rendered empty layers with exit 0.

#### Scenario: .otio flows
- **WHEN** `new` → `demo` → `roundtrip` → `convert` → `render .ktproj`
- **THEN** valid OTIO; the demo with a transition and markers; roundtrip is byte-stable; the legacy conversion works; `.ktproj` is rejected with a hint
- **TEST** t151 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Without --force it does not overwrite, with --force it is atomic
- **WHEN** `new`/`demo` onto an existing file
- **THEN** a refusal mentioning `--force`, the content is intact; with the flag the file is written, and no `.tmp` remains
- **TEST** t387 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The demo renders out of the box
- **WHEN** `demo` and then `render`
- **THEN** the project refers to `synth:`, the wav is > 1 MB; `--files` returns the former file names
- **TEST** t388 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A relative path to the project
- **WHEN** `kt-cli export sub/p.otio …` with the media `clip.mp4` next to the project
- **THEN** without "decode failed", `missing layers 0`
- **TEST** t307 (`src/apps/kt-cli/tests/cli.rs`), t34 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`) — opening with the media index does not rescan the file again

### Requirement: `kt-cli probe` — "what sounds at this point"
`probe <project> <from_s> <to_s>` SHALL print cleanly from the model, without decode and without audio
hardware: the effective events AFTER the morpher layers, the prefetch order, on a separate line the foreign
media in the order, the number of events with warp, the cost of assembling the engine, a video-order section with the
native size, the decode step and size per clip, the tree of reels with the path of each nested item, and the
ACTUAL content window of the instance. `--time` SHALL open the real sources and
print TWO numbers — the share of real time of the linear run and the REPEAT of the region through
`BurstCache`, as in the program; unavailable media is skipped in probe with a warning.
For the complaint "here the sound/picture is from another event" probe is run FIRST; judge
speed ONLY by the release build (debug is 9 times slower).
Why: the first wave of analysis of the owner's complaint looked for the cause in the engine, because there was
no way to ask "which media is playing here"; with the tool the defect was found in one run.

#### Scenario: Smoke test of the sections
- **WHEN** probe on the demo project over the first seconds
- **THEN** there are `effective clips`, `no foreign media in the order`, `clips with a time warp`, `video decode order` with `stage x`, the tree of reels, and `is a NESTED REEL` for the nested item
- **TEST** t290, t318, t452b (`src/apps/kt-cli/tests/cli.rs`) — the behaviour of the layers itself is guarded by kt-model (capability `morpher`); a test of printing does not replace it

### Requirement: Cross-render against a foreign renderer — gated conformance
Foreign .otio files from upstream SHALL be rendered by us (`kt-cli export … libx264`) and by melt
(`real_time=0`), and the frames SHALL be compared by colour classification against the table of conventions
BT.601/709 × limited/full; the tiers are strict (pixel by pixel) and smoke (both did not fail,
the durations matched). A real film is compared by a SELF-CALIBRATING metric: our frame resembles
the melt frame of the same moment more than the frame one and a half seconds away does; the melt profile is the fraction
24000/1001. Without tools the tests MUST be skipped with a clear reason;
`KT_REQUIRE_CROSS_RENDER=1`/`KT_REQUIRE_OC_RENDER=1` turn the skip into a failure. The media and
.otio of the film are not committed (script `fetch-oc-media.sh`).
Why: a match of JSON is not yet a match of the edit — a shifted cut is visible only in
the picture; a pixel-by-pixel comparison of real video is impossible (two encoders), and the automatic
encoder choice of ours could slide into nvenc and add noise.

#### Scenario: Upstream synthetics
- **WHEN** the samples simple_cut/multiple_track/preflattened/multitrack/premiere/screening are rendered by both programs
- **THEN** the colour classes match at the probe points outside the transition windows and within ±1 frame of the cuts; files with transitions and nested Stacks are the smoke tier — melt drops them, and we do not diverge
- **TEST** t158 (`src/apps/kt-cli/tests/cross_render.rs`)

#### Scenario: A real film
- **WHEN** the 30 s window of "Sol Levante" (H.264, B-frames, 23.976 fps, timecode from the hour) is rendered by both programs
- **THEN** each point is explained by a melt frame within two frames (drift of the 24 versus 23.976 grid is allowed, a shifted cut is not); the durations are compared in seconds
- **TEST** t179 (`src/apps/kt-cli/tests/oc_render.rs`)

### Requirement: Export in a foreign resolution arrives at the same fractions
Export at a resolution different from the project raster SHALL compute the layer geometry in fractions
of the export canvas by the same function as the preview: the export frame is similar to the preview composite
within the tolerance of the resampling class, and the conform rule and the node position are not recomputed
by hand. The golden on matching proportions MUST stay byte-for-byte without regeneration.
Why: before R.62 "Max (W×H)" added the maxima of width and height independently and offered a resolution that no source had, and the result was stretched.

#### Scenario: Similarity of preview and export at a foreign resolution
- **WHEN** a 1920×1080 project with a vertical source is exported at 1280×720
- **THEN** the layer rectangle is proportional to the 1920×1080 preview within the tolerance, the margins are in place; t715 is green
- **TEST** t728b (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: Mute and solo act on the file and are listed before start
Export SHALL apply track mute and solo the same way as the preview (the same renderers). The render window
SHALL list the muted tracks and the active solo BEFORE the start, in the same place
where the unavailable sources are listed, with the address "where to change" (the track header);
kt-cli SHALL print the same before the render. There is no refusal — the owner's decision of R.62: mute and
solo are the content of the project.
Why: "forgot to clear solo — delivered one track" is a risk the owner accepted; the price is the duty to say so
(rules 16, 19; Kdenlive #498560 asks for exactly this).

#### Scenario: The window and kt-cli list
- **WHEN** the project has one track muted and two in solo; the render window is open; `kt-cli render`
- **THEN** the list shows both things with the track names and the address; the render goes on; kt-cli prints a notice; the file does not contain the muted track
- **TEST** t740 (`src/apps/kt-app/src/layout_tests.rs`), t740b (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: kt-cli showcase builds a dense project from a directory of real footage
`kt-cli showcase <out.kt> --sources <dir> [--force]` SHALL write a valid project file built
from the media files found directly in `<dir>` (non-recursive, ordered by file name, media
durations and streams probed through the same media layer the editor uses). Video files
feed the picture tracks; audio-only files (music) feed the music beds and never land on a
picture track — without any, the beds come from the longest videos with sound. The project
SHALL contain, at minimum: three named video tracks and three named audio tracks plus one
morph track; on the main video track a montage of at least 24 events drawn from every source
file, with linked audio partners and at least three different transition kinds; at least one
overlay event with non-default on-screen geometry, at least three different video FX, at
least one title generator event; on a music track an audio event carrying Slowed+Reverb
whose `rate` and `mix` parameters are each automated by a curve of at least four keys; one
morpher batch; at least four named markers; a region with loop enabled; a media bin grouped
by role. Media SHALL be referenced by absolute path and MUST NOT be copied. For the same
directory contents the output SHALL be byte-identical between runs (no wall-clock, no
randomness without a fixed seed). A missing directory, one without any readable video file, or one where no video
file has an audio stream (the music bed needs one) SHALL fail with a non-zero exit code
and a message naming the directory; an existing
output file MUST NOT be overwritten without `--force`, exactly as for `new`/`demo`. Project
settings SHALL be 1920×1080 at 25 fps regardless of source resolutions, so sources of
different size and aspect exercise the conform rule.
Why: the README must show a timeline "packed with all kinds of editing" cut from real
footage (owner, 2026-10-21); a hand-built project can neither be re-created after a format
change nor re-shot after a UI change, and `demo` must stay synthetic (t388).

#### Scenario: Showcase from fixture media is rich, valid and deterministic
- **WHEN** `kt-cli showcase` runs twice on a directory holding copies of the generated fixture videos (`src/fixtures/out/*.mp4`) and one audio-only fixture (`sine48k_stereo.flac`)
- **THEN** both runs exit 0 and the files are byte-identical; the reloaded project validates; it has ≥ 24 main-track events from every source, ≥ 3 transition kinds, an overlay with geometry, ≥ 3 video FX kinds, a title generator, a Slowed+Reverb event on the audio-only file with `rate` and `mix` curves of ≥ 4 keys each (the music file is on no picture track), a morpher, ≥ 4 markers, a looped region; every media path is absolute and points into the directory
- **TEST** t824 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Missing or empty sources and existing output are refused
- **WHEN** `showcase` gets a non-existent directory / a directory with no video file / an existing output path without `--force` (the no-audio refusal shares the code path of the no-video one)
- **THEN** non-zero exit, message names the directory (or mentions `--force`), the existing file is untouched; with `--force` it is written atomically and no `.tmp` remains
- **TEST** t825 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: A forced encoder outside the system's cascade is a named error
When an export forces a video encoder that this system's cascade never tries (`h264_vaapi` on
Windows, `h264_mf` on Linux), the export SHALL fail before writing a file, with an error naming
the encoder and the cascade it is not part of ("`h264_vaapi`: not an encoder this system tries
(h264_nvenc, …)"). Why: the loop over the cascade skipped every candidate and the error read
"no video encoder could be opened (last error: )" — on windows-latest t962, which forced VAAPI,
failed on that empty string.

#### Scenario: The other system's encoder is forced
- **WHEN** an export forces the encoder of the other system's cascade
- **THEN** it fails, the error names the encoder and lists this system's cascade, and no file is
  left behind
- **TEST** t1063 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A forced VAAPI on a system without it
- **WHEN** t962 forces `h264_vaapi` on a system whose cascade has no VAAPI (Windows)
- **THEN** the error names that, and the device half of the test (which needs VAAPI in the
  cascade) does not apply; on Linux the device half runs as before
- **TEST** t962 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A frozen path never calls the platform's libm
Every transcendental function an audio or pixel kernel, a generator, a curve or the engine calls
on a path whose bytes are frozen by a reference SHALL come from `kt_plugin_api::math`
(`FrozenMath`: `fsin`, `fcos`, `fsin_cos`, `ftan`, `fexp`, `fpowf`, `flog10`, `fatan2`, `fasin`,
`fmul_add`), never from `f32`/`f64`'s methods, which call the platform's libm. The `f32` functions
are the `f64` ones of the pure-Rust `libm` crate rounded once to `f32` — correctly rounded except
on about 2⁻²⁸ of inputs — and every function stays within one ulp of glibc. The warp's sinc
table and dot product are ours as well (`kt-fx/src/audio/sinc.rs`): rubato's table on our math
and its AVX+FMA product as the one rounding, the portable path bit-equal. Exact functions
(`sqrt`, `floor`, `rem_euclid`, `powi`…) are not concerned. This is invariant 11 across platforms:
the same project gives the same audio bytes on Linux and Windows and on any CPU.
Why: on 94e36e9e windows-latest failed t973c (slowed_reverb, phaser, pitch_shift), t1019 (flip)
and t1048 (smpte, leader) — the references were printed with glibc, and mingw/msvcrt round some
inputs differently. The owner (01.11): "own math, close to Linux, where Linux is the more
accurate one".

#### Scenario: The frozen math stays within one ulp of glibc
- **WHEN** t1065 sweeps 200 000 inputs of every function over the ranges the kernels use and
  beyond, on Linux with glibc
- **THEN** every result is within one ulp of glibc's; the share that differs is printed per
  function (the `f32` ones differ only where glibc itself is not correctly rounded)
- **TEST** t1065 (`src/libs/kt-plugin-api/tests/math.rs`)

#### Scenario: No frozen crate calls the platform's functions
- **WHEN** t1066 scans kt-time, kt-model, kt-plugin-api, kt-fx, kt-text, kt-media, kt-engine and
  every plugin family for `x.sin(`, `f64::powf(` and their kin outside comments (its own scanner
  first proven on lines it must and must not name)
- **THEN** it finds none
- **TEST** t1066 (`src/apps/kt-cli/tests/frozen_math.rs`)

#### Scenario: The warp's product does not depend on the CPU
- **WHEN** the warp's sinc product runs on the AVX path and on the portable path over the same
  table and wave
- **THEN** the results are equal bit for bit
- **TEST** t1067 (`src/libs/kt-fx/src/audio/sinc.rs`)

#### Scenario: The audio kinds' sound is the same on Linux and Windows
- **WHEN** t973c renders every audio kind's four cases on Linux and on windows-latest
- **THEN** both give the table's hashes; eight of them were re-based by this change and equal
  what windows-latest printed for them on 94e36e9e, no other value moved; t1019 and t1048 pass
  on both platforms unchanged
- **AND** the export reference t11 has one audio value for both platforms — the one windows-latest
  had measured — and its video value did not move
- **TEST** t973c (`src/libs/kt-engine/tests/plugin_sound.rs`), t11
  (`src/libs/kt-engine/tests/golden_export.rs`), t1019
  (`src/libs/kt-engine/tests/trans_plugin.rs`), t1048 (`src/libs/kt-media/tests/gen_plugin.rs`)

### Requirement: The announced frame total is the frames the export renders
The frame total an export announces (`ExportProgress.total_video_frames`) SHALL be computed by the
same rule the render loop follows: the frames of the project grid from the frame containing the
range start up to, not including, the range end (`kt_engine::export::video_frames_in`). On any
range — including one whose edges are not on the frame grid — the frames rendered SHALL equal the
total, and no progress step SHALL report more frames than the total. The render progress window
SHALL compute its remaining time and estimated size from a fraction clamped to [0, 1] and SHALL
NOT build a `Duration` from a negative or non-finite number; a counter mismatch costs a wrong
estimate, never the program.
Why: the owner's render crashed the program (2026-10-31, kkt-26.11.01 item 9): the total was
floored `(r1 − r0) / step` while the loop ran from `align_floor(r0)`, so an off-grid region rendered
one frame more than announced, the fraction went above 1, the remaining time came out negative,
and `Duration::from_secs_f64` panicked on the UI thread.

#### Scenario: An off-grid region renders exactly the announced frames
- **WHEN** a 30 fps project is rendered over a region starting half a frame after a grid line and
  ending a third of a frame after another (and over an on-grid region, an empty one and one
  shorter than a frame)
- **THEN** the number of frames the raw video render delivers equals `video_frames_in` of the
  region every time, and the progress the render reports never exceeds it
- **TEST** t1070 (`src/libs/kt-engine/src/export.rs`)

#### Scenario: The progress window survives frames beyond the total
- **WHEN** the render progress window is fed a running job whose progress reports more frames than
  its total, and then a job with zero frames done
- **THEN** the window draws without a panic, "Remaining" reads 0:00 for the first job and is absent
  for the second, and the estimated size is never below the size written so far
- **PATH** File › Render… → Render → the progress window counts frames past the region's grid edge → the "Remaining" row reads 0:00 instead of the program closing
- **TEST** t1071 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A render of any region finishes and the program lives on
A render started from the render window over any region — including a short one whose edges are off
the project's frame grid — SHALL run to its finish, the window SHALL show the finished summary, and
Close SHALL close the window while the program keeps running: no panic, no crash window, no crash
report. The progress the window receives SHALL never report more frames than the announced total,
and the frames rendered SHALL equal it.
Why: the owner (kkt-26.11.01b item 2, build 26.11.01-a04b201d): "it reaches 100 % and the program
folds … even 0.2 s anywhere". His build announced the floored frame count; the window panicked on
the negative remaining time during the audio phase. The cause was fixed on master by
render-crash-and-recovery after his build; this requirement holds the whole path end to end.

#### Scenario: A 0.2 s off-grid region renders to the end and closes
- **WHEN** a 24 fps project's region starts a third of a frame after 1 s and lasts 0.2 s, and the
  render window starts a real render of it and is drawn every frame until the result
- **THEN** no progress step reports more frames than the total, the render succeeds with exactly
  the 6 frames from the one containing the start (the owner's build announced 4), the finished window
  draws, Close removes it, and the file is an MP4
- **PATH** the region drawn → File › Render… → Render… → the bar fills to 100 % → the summary line, Open file, Open folder and Close → Close → the timeline again, the program running
- **TEST** t1312 (`src/apps/kt-app/src/layout_tests.rs`); live scenario `src/scripts/live/renderregion.py`

### Requirement: An export runs its stages concurrently and writes the serial export's bytes
An export to a file SHALL run its stages concurrently inside the one output file: decode a
memory budget ahead of the composite with a thread per source, the composite on its own thread
(one device, with frames in flight between submit and readback) or on frame workers (several
devices of a software adapter, one per worker), the RGBA→encoder conversion on frame workers, the encoder and the muxer on
one writer, and the audio on its own thread. The stages SHALL be joined by bounded queues that
keep frame order, and the encoder SHALL receive exactly the frames and samples, in exactly the
order, of the serial export (`ExportOptions.serial`, the former one-thread export kept as the
reference); the writer SHALL interleave the audio by a fixed rule (after video frame `i`, the
audio up to that frame's end), so the call order into the muxer and the progress events do not
depend on timing. A frame being composed SHALL never be evicted before its composite (decode
waits while the unwritten chunks' frames fill the look-ahead budget). A cancel or an error in
any stage SHALL stop every stage promptly and return the cause. The video encoder SHALL be opened
with its own threads (`threads=auto`; the serial reference `1`), named in the open report. `kt-cli export --time` SHALL print the busy time of every stage and
the wall time; `--serial` runs the reference.
Why: the owner (2026-11-03, kkt-26.11.03 C1): "our final render is VERY slow, slower even than the
preview; it seems not parallel but strongly sequential". It was: one thread did decode →
composite → readback (a full wait per frame) → conversion → encode, libx264 encoded on one
thread, and the audio pass started after the last video frame. The showcase export took 324 s on llvmpipe
and 861 s on the since-deleted CPU composite, for 104 s of 1080p25.

#### Scenario: The pipelined export writes the serial streams
- **WHEN** the demo project (layers, transitions, FX, automation, audio FX) is rendered over a
  3 s off-grid range at 1280×720 — chunks of a few frames — serially and pipelined, and pipelined
  again
- **THEN** every video frame and every audio sample of the pipelined stream equal the serial
  passes', the frame count, missing layers and loudness are equal, and the two pipelined runs
  make the same calls into the sink in the same order, audio from the first frames on
- **TEST** t1451 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: A cancel or an error mid-pipeline stops every stage
- **WHEN** a pipelined export is cancelled after its tenth written frame, and another one's sink
  fails writing its sixth frame
- **THEN** the first returns "cancelled" with no frame written after the cancel, the second
  returns the sink's own error (not a stage's "the writer stopped") with five frames written, and
  both calls return — every stage's thread joined
- **TEST** t1452 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: The encoder gets its threads
- **WHEN** a two-second project is exported with libx264, pipelined and serial
- **THEN** the open report's options carry `threads=auto` and `threads=1` respectively, and both
  files hold all 60 frames and two seconds of audio
- **TEST** t1456 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The GPU readback ring gives the executed bytes
- **WHEN** the demo project is exported on the GPU serially (`execute_into` per frame) and
  pipelined on a fresh engine (submit ahead, collect in order), and six plans go through the ring
  three in flight
- **THEN** the pipelined frames equal the serial GPU frames, and every collected frame equals the
  `execute_into` of its plan, byte for byte (one device)
- **TEST** t1457 (`src/libs/kt-gpu/tests/export_ring.rs`)

### Requirement: The export run reports what it opened and what it wrote, in counts only
An in-process MP4 export (`export_mp4_with_backends`) SHALL report, through its event callback:
(1) once, BEFORE the first video frame is delivered, the video encoder that actually opened — its
name, whether it is a hardware encoder, the pixel format it is fed, and the options it was opened
with — together with every candidate tried before it and the error code each returned; (2) with
every progress step, besides the frame/sample counters and the phase, the number of encoded video
bytes and audio bytes handed to the muxer so far; (3) log lines: each candidate that did not open,
each source rendered as a placeholder, each phase change, the start of finalization and any error.
The opened encoder's name SHALL equal `ExportStats.encoder`. No event SHALL carry a wall-clock
value: two runs of the same export on the same machine SHALL produce the same sequence of events
(the time of each step, the elapsed time, the speed and the remaining time are computed by the
caller). `kt-cli export` SHALL print the log lines to stderr.
Why: the muxer chooses the encoder before the first frame (`Muxer::open` in
`src/libs/kt-ffmpeg/src/muxer_impl.rs`), but before this rule only the final stats name it, so the progress
window calls it "auto encoder" for the whole run, and a silent fall-back from hardware to
software is learnt only at the end (F-EXP-004 Cons). Invariant 11 forbids the wall clock in the
offline path: counts are deterministic, clocks are not.

#### Scenario: The encoder is named before the first frame and the bytes add up
- **WHEN** a 60-frame export at 30 fps is run with the encoder forced to `libx264`, and again with
  the encoder left to the cascade
- **THEN** in both runs the first event that is not a log line is the opened-encoder report and it
  precedes every video progress step; with `libx264` it names `libx264`, software, `yuv420p` and
  options containing `crf`; with the cascade its name equals `ExportStats.encoder` and the log
  holds one "not opened" line (name and error code) for every candidate of the OS list placed
  before it that exists in the loaded FFmpeg; the video and audio byte counters never decrease, the
  last ones are greater than zero, and their sum is not larger than the size of the finished file; the last progress step
  counts every video frame and audio sample of the totals
- **TEST** t954 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The event stream holds no clock
- **WHEN** the same project is exported twice with `libx264` into two files and both event streams
  are recorded
- **THEN** the two streams are equal element by element (encoder report, every progress step with
  its counters and bytes, every log line), and no log line contains a time of day or a duration
- **TEST** t955 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: The render progress window shows the output settings, the run and the speed
While a render runs, the progress window SHALL show two labelled forms side by side (one under the
other when the window is narrower than both): **Output** — resolution, frame rate, video (the name
of the encoder that opened, its pixel format and its quality setting under the name that encoder
uses — CRF, CQ, QP or global quality — or the target bitrate), audio (codec, sample rate, channels,
bitrate), mux (container and `+faststart`), backend (GPU with the adapter name, or "no device" with
the reason) and the output file path; **Run** — phase, progress as done/total
frames, elapsed, remaining, output length rendered/total, file size so far, the estimated final
size and the current video bitrate. Until the encoder report arrives the video row SHALL say that
the encoder is being chosen, never a guessed name. Below the forms: the speed line (min, avg, cur,
max × real time) and a speed chart drawn by the program's rolling graph; then the progress bar;
the Pause/Resume and Cancel buttons SHALL stay outside the scrolling body. The window SHALL
repaint only while the job runs (invariant 12).
Why: the owner's request to bring the window at least to v1's level of information (2026-10-27);
v1's `RenderProgressDialog` had these forms and a speed chart
(`git show cpp-reference-final:renderprogressdialog.cpp`), but its video row was a fixed string
"H.264 (libx264), yuv420p, preset veryfast, CRF 23" — we take the rows, not the fixed text
(F-EXP-005 trick 1). The quality knob means a different regulator per encoder (F-EXP-004 trick 3).
Speed and the bitrate are per second of OUTPUT so they are comparable between projects.

#### Scenario: The rows come from the run
- **WHEN** the harness opens the progress window of a job fed scripted engine events: first only a
  progress step, then the encoder report `h264_nvenc` (hardware, `yuv420p`, `cq 23`), then
  progress steps with byte counts, with the backend "GPU: llvmpipe"
- **THEN** before the report the video row says the encoder is being chosen and contains no
  encoder name; after it the Output form reads the project's resolution and frame rate, "h264_nvenc"
  with "hardware", "yuv420p" and "CQ 23" (not "CRF"), "AAC, 48 kHz stereo" with the bitrate,
  "MP4 (+faststart)", "GPU: llvmpipe" and the file path; the Run form has the phase, "N / M" frames,
  output length, the size and the bitrate computed from the scripted bytes and frames; the speed
  chart rectangle is published and non-empty; Pause and Cancel lie outside the scrolling body; no
  window outside the `WINDOWS` registry is open
- **PATH** Render… → Render settings → Render… → save dialog → name → the progress window: on
  the left "Output" with resolution, fps, "Video: h264_nvenc (hardware) · yuv420p · CQ 23", audio,
  mux, "Backend: GPU (…)", the file; on the right "Run" with phase, frames, elapsed, remaining,
  output length, size, bitrate; under them the speed line and the chart moving to the right; the
  bar; Pause and Cancel at the bottom
- **TEST** t956 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `render` (`src/scripts/live/render.py`)

### Requirement: The finished render is frozen, summarised and opened from the window
When the job finishes, the window SHALL stop every time-based number at the moment the result
arrived (elapsed, speed, remaining) and SHALL show a final block: total time, average speed,
frames and audio samples written, encoder, backend, file size and average bitrate, followed by the
existing loudness, missing-source and lost-layer lines. A successful render SHALL offer **Open
file** (the file in the system's default application) and **Open folder** (the file manager at the
file's folder); a failure SHALL offer opening the render log. The button that closes the window
SHALL NOT occupy the place where Cancel stood during the run. While the phase is Finalize, Cancel
SHALL be disabled with a hover hint saying the file is being finished. A failure to open the file
or the folder SHALL be reported as an error notice naming the path.
Why: ktprompts v26.10 T12 "the render window counts statistics after finish" — `elapsed` is read
from `BgJob::started` every frame, so after the end it keeps growing and the average speed sinks.
Vegas turns Cancel into Close in place — a late click cancels or closes by surprise, and a cancel
during the final mux throws away a finished file (F-EXP-005 trick 3).

#### Scenario: Counters stop at the finish and the next actions appear
- **WHEN** the harness drives a scripted job to Done with an injected clock: the result arrives at
  10 s, then two more frames are drawn at 11 s and 15 s; separately, a job is held in the Finalize
  phase; separately, a job ends with an error
- **THEN** at 11 s and 15 s elapsed, average speed and the final block read exactly what they read
  at 10 s and "remaining" is gone; the final block names total time, average speed, frames,
  encoder, backend, size and average bitrate; Open file and Open folder are present and neither
  lies on the rectangle Cancel had; in Finalize, Cancel is disabled and its hover hint names the
  finishing step; after the error the window offers the log and no Open file
- **PATH** wait for the bar to fill → the phase says "Done", the numbers stop moving; the summary
  "Rendered in 0:42 · 2.1× real time · 1260 frames · h264_nvenc · GPU · 48.3 MB · 9.2 Mbit/s"
  and the loudness line → Open folder → the file manager shows the folder holding the file
  → Open file → the video plays in the system player → Close
- **TEST** t957 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `render` (`src/scripts/live/render.py`)

### Requirement: Rendering over an existing file is asked first
When the output path chosen for a render already exists, the program SHALL ask before starting,
in its own question surface (the `Ask` set, not a new window): the question SHALL name the file,
its size and its modification date, and offer **Replace** and **Choose another name**; the
question SHALL say that the name is chosen again in Render… (rule 19). Nothing SHALL be written,
truncated or deleted before Replace. Choose another name SHALL reopen the save dialog with the same
default name; closing the question SHALL return to the settings window with every setting kept. A
path that does not exist SHALL start the render with no question.
Why: ktprompts v26.10 U1 "render to an existing file — ask"; today `launch_render` passes the
path straight to the export, and the muxer's `avio_open` truncates the old file at once — a
cancelled or failed render then also deletes it (`cleanup_on_err` removes the output path).

#### Scenario: An existing file is not touched without Replace
- **WHEN** the harness points the save seam (`RenderSetup.test_pick`; the live scenario uses
  `KT_FILE_PICK`) at an existing file of known bytes and
  presses Render…; answers "Choose another name"; then again and answers Replace; separately points
  it at a path that does not exist
- **THEN** the first press opens the overwrite question naming the file, its size and date, and the
  file's bytes are unchanged; "Choose another name" returns to the save step and the file is still
  unchanged; Replace starts the render (the progress window opens); the non-existing path starts
  the render with no question; no window outside the `WINDOWS` registry opened
- **PATH** Render… → Render settings → Render… → the save dialog → pick `kt-demo-12s.mp4`, which
  is already there → the question "kt-demo-12s.mp4 already exists (48.3 MB, 2026-10-27 14:02).
  Replace it? The name can be chosen again in Render…" with Replace and Choose another name →
  Choose another name → the save dialog again → the same file → Replace → the progress window
- **TEST** t958 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `render` (`src/scripts/live/render.py`)

### Requirement: The render log is kept in the window and in one file
The progress window SHALL hold a collapsible log section (collapsed while the run is healthy,
opened by itself when the run ends in an error) with at most the last 800 lines; each line SHALL
be prefixed by the UI with the elapsed time of the run. The log SHALL contain the engine's log
lines, the backend choice and its reason, the sources rendered as placeholders, the Pause, Resume
and Cancel requests and the result. The same lines SHALL be recorded in the telemetry journal, which
mirrors them into the run's trace; no other file SHALL be written for them (no `render.log`). After a
failure the window's Open log SHALL open the telemetry journal.
Why: v1 had an 800-line log panel and wrote a log file on error
(`projectrenderer.cpp`, `render-error-<UTC>.log`); a failed render used to leave one error line in
the window and nothing on disk. kkt-26.11.18 T4: "объедини лог рендера в телеметрию, нахуя ты его
выделил отдельно то?!?" — every line already went into the journal and the trace, and `render.log` was a
third copy the owner had to collect apart from the trace he sends.

#### Scenario: The log is bounded, journaled and named
- **WHEN** the harness runs a scripted job that emits 1000 log lines and an error, with the data
  directory in a temp folder and the telemetry journal bound to a test journal; then presses Open log
- **THEN** the window's log holds the last 800 lines, each starting with an elapsed stamp, and is
  open after the error; the test journal holds all 1001 lines as `EXPORT` records; no `render.log` or
  `render.log.1` exists in the data directory; Open log opens the telemetry journal's path; the window
  shows no log path; every text of the log section exists in the five catalogs
- **TEST** t959 (`src/apps/kt-app/src/layout_tests.rs`, `src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)

### Requirement: A real VAAPI export produces a file that decodes, proven on hardware
On a machine with a VAAPI render node whose driver encodes H.264, an export with `h264_vaapi`
forced SHALL produce an MP4 in which our own decoder reads every exported frame, and the export
statistics and the encoder report SHALL name `h264_vaapi`, hardware, and the device used. This
check SHALL run when `KT_EXPECT_HW=vaapi` is set and SHALL fail the run when VAAPI does not open
there (a skip is not a pass); without the variable it SHALL be skipped with the reason printed.
CI runners have no GPU, so the check is run by `make hw-test` where a machine with a render node
is at hand. Until then the same check SHALL run on the fake driver (`fixtures/fake_va`), which
proves the path through the real libva and FFmpeg and nothing about any real driver.
Why: "it ran green" with the check silently skipped would prove nothing — the same rule as
`KT_EXPECT_FFMPEG_MAJOR` (t865). No machine of this project has a render node, and the surfaces, the
upload and the flush had never been executed by anything.

#### Scenario: Sixty frames through VAAPI
- **WHEN** with `KT_EXPECT_HW=vaapi` a 60-frame 1280×720 export at 30 fps of the synthetic ramp
  project is run with `h264_vaapi` forced
- **THEN** the file holds an H.264 stream from which our decoder reads 60 frames over all keys of
  the grid, the mean absolute difference of each decoded frame from the raw export frame is under
  the bound recorded in `design.md`, and the statistics name `h264_vaapi` with the device path;
  with `KT_EXPECT_HW=vaapi` on a machine without a usable node the test fails naming the reason
- **TEST** t963 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The upload decodes to the frames it was given
- **WHEN** t963 itself runs in a child process on the fake driver, a regular file as the device
- **THEN** every frame reads back, and the worst frame's mean absolute difference from the raw export
  frame is at most 3 of 255: the fake encoder keeps every sample, what is left is the RGB → 4:2:0 →
  RGB round trip
- **TEST** t1556 (`src/libs/kt-engine/tests/export_options.rs`)

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

### Requirement: A frame an unwritten chunk needs is never evicted
Every frame named by a chunk of the pipelined export SHALL be held in the cache (capability
`media-index`) from the moment that chunk's decode requests are made until its last frame is written,
whether it was decoded for that chunk or found there. Decode SHALL wait while the bytes held exceed the
look-ahead budget. A frame a span did not give SHALL be asked again alone before it is composed, as the
serial export's pre-roll does. The frames, the samples and `missing_video_layers` of the pipelined export SHALL
equal the serial export's and be the same on every run, at any relative speed of the stages.
Why: the cache stays full during an export, a composite's reads refresh frames never needed again,
and these outlived frames decoded and not yet read. A different set of frames lost a layer on
each run of the demo (5, 7, 8 and 10 layers), and the file was damaged. The earlier promise of this
capability rested on the look-ahead fitting the budget, which does not decide what LRU drops; t1451
never fills the cache.

#### Scenario: Under pressure the pipelined export is the serial export, every run
- **WHEN** 180 frames of the demo at 640×360 with a quarter of the budget, over its first transitions,
  are rendered serially, and three times pipelined into a writer slow enough that decode runs at its
  look-ahead limit
- **THEN** every run equals the serial export frame by frame, with zero missing layers
- **TEST** t1601 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: A span that fails half way loses no frame
- **WHEN** a source's span stops with an error half way through a chunk, once, while each frame can
  still be read alone, and the range is exported serially and pipelined
- **THEN** no frame is written without its layer, and nothing counts as lost
- **TEST** t1605 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The file's streams equal the serial export's
- **WHEN** `kt-cli export` writes a project pipelined with `--encoder-threads 1`, and with `--serial`
- **THEN** the video packets of the two files are identical and so are the audio packets; the
  interleave of the file may differ, because the serial export writes the audio after the video
- **TEST** t1603 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: A frame written without a layer is reported where it happens
A frame composed without one of its layers SHALL be reported during the export, for the serial and
the pipelined export alike and in the same words: one log line per run of consecutive such frames,
in frame order, naming the time of the first and the last frame and the media that gave no picture.
The render window SHALL open its log by itself when the run has such a line; kt-cli SHALL print it.
The export goes on: a layer no decoder can give is not worth a lost render, and the person is told
where to look.
Why: the only trace was a count at the end ("missing layers 5"), which says neither which frames nor
which media, so a damaged file could pass for a finished one.

`missing_video_layers` SHALL count the layers frames were written without, at the frame: a span that
failed and gave its frames back is no loss, and one loss counts once.

#### Scenario: A source that fails for a stretch is named with its frames
- **WHEN** a source cannot give its frames for two stretches of a range, and the range is rendered
  serially and pipelined
- **THEN** both give the same `IncompleteFrames` events, in frame order, one per run of frames, naming
  that media and those times; the frames and the missing count are equal
- **TEST** t1602 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A layer with no source counts once
- **WHEN** a 60-frame project whose video has no source at all is exported, serially and pipelined
- **THEN** both report 60 missing layers, one for each frame
- **TEST** t1606 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A failed render's window says what failed in the person's words
The error of a failed render SHALL be the export's own message, without the name of an internal error
kind. When a run ended without opening an encoder, the window's Video row SHALL say that no encoder
opened, and SHALL NOT keep saying the encoder is being chosen.
Why: "Error: io: …" names a Rust type, and "choosing the encoder…" under a finished failure says
the program is still working on it.

#### Scenario: A run that opened no encoder
- **WHEN** a scripted render ends with an export error before any encoder opened
- **THEN** the window's Video row reads "no encoder opened", its error line carries the message with
  no "io:", and the log section is open
- **PATH** File → Render… → Render… → the window: the Video row "no encoder opened", the red line
  with the reason, the log open, Open log and Close
- **TEST** t1604 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A software adapter composes an export on several devices
An export on a software adapter SHALL open a device per frame worker: an eighth of the cores, 2 to 4 of
them, as many as the available memory holds for the frame size, and at least one. One device SHALL serve a
real card. The export SHALL USE as many of the opened devices as its measured time says: it SHALL start on
one, add one while the frames written per second of wall grow by more than a tenth, keep the fewer on a
tie, and keep re-trying one more and one fewer as the render goes, by the same rule on every machine. The
frames written SHALL NOT depend on how many devices composed them, and the export's line SHALL name the
counts it used. The video decoders of a render SHALL open with four threads, and a render short of memory
SHALL run tight: two decoder threads, half the frame cache, four encoder threads. A render SHALL NOT die
of memory; speed is secondary.
Why: each device keeps its own textures and shaders and adds 1.3 GB at 1080p, and seven decoders at auto
held 2.7 GB of frames for nothing: the showcase went from 17 GB to 8.7 GB and from 144 s to 130 s. Row
710: four devices took 31.8 s against 12.5 s on one over a light stretch and won over the whole project —
no fixed count is right for both.

#### Scenario: Several devices write the frames of one
- **WHEN** one project is exported through one engine and through three, on the same adapter
- **THEN** the frames written are byte for byte equal
- **TEST** t1573 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: The device count follows the memory and the frame
- **WHEN** the count is asked with plenty of memory, 7 GB, 4 GB, 1 GB at 4K, or memory unknown
- **THEN** it is the ceiling, less than the ceiling, one, one (never none), and two; a bigger frame gets
  no more devices than a smaller one; little memory gets two decoder threads, any other four
- **TEST** t1642 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The memory is read
- **WHEN** the program asks what it may take on Linux or Windows
- **THEN** it gets a number; on Linux a control group's limit counts
- **TEST** t1643 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The count follows the measured time
- **WHEN** the rule is fed windows whose rate grows with each device up to three of four, then a stretch
  where every count runs at the same rate, then one where two win again
- **THEN** it climbs to three and stops there, comes down to one on the even stretch, goes back up to two,
  never asks for none or more than it has, and a rise of less than a tenth adds no device
- **TEST** t2900 (`src/libs/kt-engine/src/export_pipeline.rs`)

#### Scenario: A changing count writes the frames of one device
- **WHEN** a project is exported on three devices with the count scripted to change every few frames
  (1, 3, 2, 3, 1) and on one device
- **THEN** the frames are byte for byte equal, and the line names the counts used
- **TEST** t2901 (`src/libs/kt-gpu/tests/export_ring.rs`)


### Requirement: A render goes on past the end of a cut file
A render of an event longer than its file holds SHALL give every frame, the frames past the end with
the file's last picture, and lose no layer.
Why: the render looked stalled and nothing named the cause.

#### Scenario: An event over the cut
- **WHEN** a 20 s event reads a 12 s file cut to about 6 s
- **THEN** 600 frames are written, no layer is lost, the decoder reopens at most once and decodes
  under 300 pictures
- **TEST** t1763 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A pipelined chunk is counted by what its frames ask
The chunks of the pipelined export SHALL be frames in a row up to half a budget of what those frames
ask, and one frame when it alone asks more. The serial loop keeps its measure.
Why: the project's largest size made a chunk of one frame on the showcase: the sources advanced in step
and nothing ran in parallel within a source.

#### Scenario: A stretch of few layers in a project of many
- **WHEN** the serial measure gives one frame per chunk
- **THEN** the chunks follow each other over every frame, some hold several, and none of several frames
  passes the limit
- **TEST** t1767 (`src/libs/kt-engine/src/export_pipeline.rs`)

### Requirement: An export's device keeps no upload past the frames in flight
A device that composes an export SHALL keep an uploaded source frame only while a frame in flight
(the readback ring's) or the frame being built still reads it, so a source frame used by consecutive
frames is uploaded once and nothing older stays. A texture a frame being built reads SHALL NOT be
reused for another upload of the same frame, on any device. The frames written SHALL NOT change.
Why: found-defects 10, 85, 96 — each export device kept a 256 MB upload cache for frames an export
uploads once; at 1080p that is 31 frames per device, on three or four devices.

#### Scenario: Uploads past the ring are dropped, the bytes stay
- **WHEN** twenty-four consecutive frames are composed once each on an export device and on a preview device with its full upload cache, each frame twice in a row on the export device
- **THEN** after every frame the export device holds at most the uploads of the frames in flight and the frame being built, the frame read again at once uploads nothing, the frames are byte for byte the preview device's, and a card's preview device holds more
- **TEST** t2298 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: A CPU device's preview keeps two frames of uploads
- **WHEN** twenty-four consecutive frames are composed on a preview engine of a device of type CPU, each twice in a row
- **THEN** it holds at most two frames' uploads after every frame (its textures are the process's memory, a second copy of the decoded frames: cache-audit cut 3), and the frame composed again at once uploads nothing; a card's preview keeps its cache
- **TEST** t3606 (`src/libs/kt-gpu/tests/export_ring.rs`)

### Requirement: An export claims the preview's growth while it runs
While an export runs in the process, the memory ledger SHALL grant no tenant anything above its base:
the preview's kept composites and stream history drop to their bases when the export starts, and are
granted their wants again when the last export ends. The export's own holds and caches are untouched.
Why: an export started from the editor sized its devices and its tight mode on the memory the preview's
grown memos had left (found-defects 10).

#### Scenario: The export takes the room and gives it back
- **WHEN** the preview's present memo and stream memo hold grants above their bases and an export runs
- **THEN** during the export both caps are their bases and the memos hold no more; after it every grant is back and the export's frames are the same as with no grants
- **TEST** t2299 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: A paused export waits on an event
While the render window's Pause holds, the export's frame loop and its pipeline's gate SHALL wait on a
condition that the pause, the resume, the cancel and the writer notify, never on a timer.
Why: row 271 — a paused export slept 40 ms and its pipeline waited 50 ms in a loop.

#### Scenario: A paused export does not wake until resumed
- **WHEN** an export is paused for half a second and resumed
- **THEN** its pause wait wakes at most once (the resume) and the export finishes with the same frames
- **TEST** t2261 (`src/libs/kt-engine/tests/export_pause.rs`)

### Requirement: A render's decoder settings are its own
A render SHALL open its sources with the decoder thread and RGBA thread counts it chose for its size and
the free memory; a source opened at the same time by anyone else (the preview, a thumbnail) SHALL take
the process's defaults. The settings are scoped to the thread that opens the render's sources, not set
process-wide.
Why: row 94 — the render window set the process-wide counts around its opening, and a second decoder
of a file opened meanwhile took the render's settings, or reset ones.

#### Scenario: Another thread's open takes the defaults
- **WHEN** a thread opens inside a render's settings while another thread asks at the same time, and the first asks again after its scope
- **THEN** the first sees the render's counts inside, the other and the first after it see the defaults
- **TEST** t2262 (`src/libs/kt-ffmpeg/src/lib.rs`)

#### Scenario: kt-cli export opens its sources in the scope
- **WHEN** `kt-cli export` opens its sources (found-defects 1059: it set the counts process-wide, and they outlived the export)
- **THEN** the sources open with the render's counts and the process's counts stay the defaults after it; no process-wide set is left in kt-cli but the preview tools'
- **TEST** t3602 (`src/apps/kt-cli/src/main.rs`)

### Requirement: An export composes an incomplete frame again before it writes it
Every export (serial and pipelined) SHALL ask a frame its decode span did not give alone, up to two times,
before anything composes it; a frame that still comes out incomplete SHALL have its missing frames decoded
alone again (serial) and SHALL be composed again (both, and the readback ring's collected frames), up to two
times, before it is written. Only what the input still does not give after that is written without its layer
and reported (the requirement "A frame written without a layer is reported where it happens"). An export's
frame SHALL NOT be memoized, and its point-wise runs take the settled form (video-plan).
Why: found-defects 522 — catch-up-shows-loading D5 made a blend pair lacking a side an absent layer and said
"the export retries"; the serial export asked a missing frame once (its pre-roll) and the pipelined one once
(alone after the span), then wrote the frame without the layer. The serial export also composed through the
memo path.

#### Scenario: A read that failed is not a lost layer
- **WHEN** a source whose every frame fails its first two reads (in a span or alone) is exported serially and pipelined
- **THEN** both report no missing layer and no incomplete run, and the two files' video is the same
- **TEST** t2518 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A stream stage takes the pipelined export
A project with a stream stage SHALL export through the pipeline: the range's history pre-rolled on the
decode thread, the plan of each frame built in frame order on the compose thread and handed to a worker;
the frames SHALL be the serial export's.
Why: found-defects 490 — a render with a Datamosh ran serially, the device and cores idle between frames.

#### Scenario: The pipelined export walks the stream in order
- **WHEN** a cut mosh is exported pipelined on one device and on three workers, over chunks of four frames, from a range before the stream and from one inside its history
- **THEN** no frame loses a layer and the frames equal the serial export's
- **TEST** t2707 (`src/libs/kt-engine/tests/stream_stage.rs`)

### Requirement: The stream question builds nothing
Whether a project has a stream stage SHALL be asked of the project's slots (`project_may_stream`:
events of every reel and morpher patches), never of a renderer built only to answer; it SHALL say yes
wherever a build would. (Since stream-round-trips the export no longer asks it: a stream stage takes
the pipelined export.)

#### Scenario: Slots, not a build
- **WHEN** a project without a stream slot, with one on an event and with one inside a reel is asked
- **THEN** no renderer is built and the answer equals the build's `has_stream_stages`
- **TEST** t2668 (`src/libs/kt-engine/tests/frame_cost.rs`)

### Requirement: The output format reaches the file
An export SHALL write the container, video codec, rate control, keyframe interval, B-frames, bit
depth, colour tags, audio codec and metadata of its `OutputFormat` (`ExportOptions.format`), and a
frame-rate override (`ExportOptions.fps`) SHALL set both the stream's rate and the number of frames
rendered. The containers are MP4, MOV, MKV and WebM; the video codecs H.264, H.265, AV1, VP9 and
ProRes 422 HQ; the rate modes Quality, VBR (an average and a maximum) and CBR; the audio codecs AAC,
PCM 16, PCM 24, FLAC, Opus and none. A container takes only the codecs of its table (MP4:
H.264/H.265/AV1/VP9 with AAC; MOV: H.264/H.265/ProRes with AAC/PCM; MKV: all; WebM: VP9/AV1 with
Opus); another pair SHALL be refused before any file is created. The default format SHALL be the
file the export wrote before this change: MP4, H.264, Quality 23, encoder speed `veryfast`, a
two-second keyframe interval, no B-frames, 8-bit 4:2:0, AAC — now carrying the colour tags of D6.
H.265 in MP4/MOV SHALL be tagged `hvc1`. No creation time is written, and Matroska/WebM are written
bit-exact (their muxer otherwise draws a random segment id per file) — invariant 11. The keyframe
interval is the encoder's MAXIMUM: a scene cut may bring a keyframe earlier.
Why: kkt-26.11.15 N5 — "очень скудные настройки финального рендера"; F-EXP-006 decides each option.

#### Scenario: The chosen settings are read back from the written file
- **WHEN** a one-second project is exported with libx264 VBR 4 Mbit/s max 6 Mbit/s, keyframes every
  0.5 s and 2 B-frames into MKV with FLAC and a title; with libx265 10-bit Quality into MP4; with
  libvpx-vp9 into WebM with Opus; with prores_ks into MOV with PCM 24; with AAC disabled ("none")
- **THEN** our own libav reader of the output reports the container, the codec, the profile (High,
  Main 10 with `hvc1`, VP9 profile 0, ProRes HQ intra-only), the pixel format, no gap between
  keyframes longer than 15 frames of the 30 fps stream, B-frames present only where asked, the audio
  codec and its 24 bits, the title and comment, a video bitrate under 1.25× the VBR maximum, no
  audio stream for "none", and the default file's BT.601 limited tags; an encoder this FFmpeg lacks
  is reported as skipped by the test, never passed
- **TEST** t2802 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: A frame-rate override renders that many frames
- **WHEN** a 30 fps project of one second is exported with `fps = 25`
- **THEN** the progress total and the written stream are 25 frames at 25 fps
- **TEST** t2804 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: A container refuses a codec it cannot hold
- **WHEN** ProRes is asked in MP4, or AAC in WebM
- **THEN** the export fails before creating the file, naming the container and the codec
- **TEST** t2802 (`src/libs/kt-engine/tests/render_settings.rs`)

### Requirement: Each codec has its own cascade, and what a candidate cannot do is named
Every video codec SHALL have a hardware-first cascade per OS tried with a real `avcodec_open2`
(invariants 9, 10). A candidate that cannot take the asked bit depth or rate mode SHALL be skipped
with that reason in the open report (and so in the render log); a forced encoder that cannot SHALL
fail with that reason. The render settings window SHALL show a codec, an encoder, a bit depth, a
rate mode, an audio codec or a container the loaded FFmpeg cannot open or the chosen container
cannot hold as unavailable, with the reason, instead of hiding it.
Why: F-EXP-004 — the list a person sees must be what will open; F-EXP-006 — an option silently
dropped is a file the person did not ask for.

#### Scenario: The cascades and the container table are fixed data
- **WHEN** the candidates of every codec are listed for Linux and Windows, and every container is
  asked about every codec and audio codec
- **THEN** each list is hardware first and ends with a software encoder; ProRes is only
  `prores_ks`; MP4 refuses ProRes and PCM, WebM refuses H.264 and AAC, MKV takes everything
- **TEST** t2801 (`src/libs/kt-ffmpeg/src/encoders.rs`)

#### Scenario: A candidate without 10-bit or CBR is skipped with the reason
- **WHEN** H.264 10-bit is exported with Auto, and `h264_vaapi` is forced at 10-bit; then libx264
  CBR 500 kbit/s is exported
- **THEN** the Auto export's report lists every hardware candidate present here as skipped with
  "no 10-bit input" and libx264 writes `yuv420p10le`; the forced one fails naming
  "h264_vaapi: no 10-bit input"; the CBR file's rate is within 25 % of the asked one
- **TEST** t2803 (`src/libs/kt-engine/tests/render_settings.rs`)

### Requirement: The colour tags are the conversion's
The RGB→YUV conversion of an export SHALL use the chosen matrix (BT.601 or BT.709) and range
(limited or full), and the stream SHALL be tagged with the same matrix and range, with BT.709
primaries and transfer. The default SHALL be the matrix by the file's size, as the decode reads an
untagged source (`colour::is_hd`): BT.709 at HD and above (1280 wide or taller than 576), BT.601 below,
limited range; a chosen matrix stays at any size. The text form names the default `matrix=auto`.
Why: the files carried no tags at all; a player guessed BT.709 for HD and showed different colours
than the preview (F-EXP-006 trick). Found-defects 1054: the default was BT.601 at every size, and a
player that ignores tags showed an HD file's colours shifted; most HD deliveries are BT.709 (the
owner, kkt-26.11.20 Q50: "709 от HD, 601 ниже").

#### Scenario: A saturated colour comes back by the tag
- **WHEN** a flat saturated red frame is exported with BT.601 limited, BT.709 limited and BT.601 full
- **THEN** the reader reports the three tag pairs (smpte170m/bt709 matrix, limited/full range,
  BT.709 primaries and transfer); every file reads back as written within 6 codes through the
  decode that reads the tags (source-colour-matrix) — a file converted with one matrix and tagged
  with another would come back 12 or more codes off, so the conversion follows the label
- **TEST** t2806 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The default matrix follows the size
- **WHEN** the default settings export 1280×720 and 640×360, and the default and a chosen BT.601 are asked at 1920×1080, 720×1280 and 720×576
- **THEN** the HD file is tagged BT.709 and the SD one SMPTE 170M; the default answers BT.709 for 1920×1080 and the portrait 720×1280, BT.601 for 720×576; the chosen BT.601 stays BT.601 at 1920×1080; the text form writes and reads `matrix=auto`
- **TEST** t3603 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: A fractional frame rate and a frame's own time are written as they are
- **WHEN** 48 frames are written at the rate 24000/1001, and five frames at their own times 0, 3000, 9000, 10000, 19000 in a time base of 1/90000
- **THEN** the first stream's rate is 24000/1001 and it lasts 2.002 s (an integer grid wrote 24 and 2.000 s); the second's packets stand at those times, their gaps in the same ratios (`MuxConfig.frame_rate` is a rational, `time_base` and `write_video_frame_at` serve a proxy that keeps its source's times; found-defects 823 — the project's own rate is F-SPD-005)
- **TEST** t3601 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The same settings write the same bytes
- **WHEN** the same project is exported twice with libx265, libvpx-vp9 and prores_ks, each
  with fixed encoder threads
- **THEN** each pair of files is byte-identical (AV1 encoders are not covered)
- **TEST** t2805 (`src/libs/kt-engine/tests/render_settings.rs`)

### Requirement: One text form of the render settings for kt-cli and presets
The render settings SHALL have one text form, ordered `key=value` pairs (`container`, `video`,
`encoder`, `rate`, `quality`, `bitrate`, `maxrate`, `keyint`, `bframes`, `speed`, `depth`,
`matrix`, `range`, `audio`, `abitrate`, `size`, `fps`, `title`, `comment`), parsed back to the same
settings; an unknown key or a bad value SHALL be an error naming it. `kt-cli export` SHALL take them
as `--set key=value` and as `--container`, `--video`, `--rate`, `--quality`, `--bitrate`, `--audio`,
`--size`, `--fps`; the third positional argument stays the forced encoder.
Why: the GUI and the CLI must be able to ask the same file (export spec, kt-cli).

#### Scenario: The pairs round-trip and refuse what they do not know
- **WHEN** every option is set to a non-default value, written as pairs and parsed back; then
  `speed=warp` and `colour=red` are parsed
- **THEN** the settings are equal; the two errors name `speed` and `colour`
- **TEST** t2800 (`src/libs/kt-engine/src/export_settings.rs`)

#### Scenario: kt-cli writes what its flags ask
- **WHEN** `kt-cli export demo.kt out.mkv libx264 --container mkv --audio flac --set keyint=1
  --set title=Demo --fps 25`, then `--set speed=warp`
- **THEN** the file is MKV with H.264 at 25 fps, FLAC audio and the title; the second run exits
  non-zero naming `speed`
- **TEST** t2808 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: Render presets and the last settings live between projects
Named render presets SHALL be kept in the user store (`render-presets.json`) with the pairs of the
text form; factory presets SHALL be named by codec, size and rate, never by a platform. The last
settings SHALL be stored when a render starts and restored the first time the window opens in a
session. A record with a key this build does not know SHALL be shown as skipped and kept verbatim
when the file is saved; a broken file SHALL be moved aside at the first write; a newer version is
read-only.
Why: the settings reset at every launch and every project open (F-EXP-001 "what is missing").

#### Scenario: Save, recall, survive a future key
- **WHEN** a preset is saved, the store is reopened, the preset is chosen; a record carrying
  `hdr=pq` is in the file; a preset is deleted; a render starts
- **THEN** the chosen preset sets every option; the `hdr` record is listed as skipped and is still
  in the file after the save; the deleted one is gone; `last` holds the started settings
- **TEST** t2809 (`src/apps/kt-app/src/user_store.rs`)

### Requirement: The render settings window offers every option, its range and an estimate
The render settings window SHALL offer, in groups, the preset, the video codec and encoder, the rate
mode with its numbers, the encoder speed, the keyframe interval, the B-frames, the bit depth, the
colour, the size (with a custom W×H) and the frame rate, the audio codec and bitrate, the container,
the range (whole project or the region, shown with its times) and the title and comment; it SHALL
show an estimated size when the rate is known and say "depends on the picture" when it is not. The
file name SHALL keep the owner's template with the container's extension. With the window open and
nothing moving the program SHALL request no repaint.
Why: N5; rule 16 (the range the render takes was not shown); the idle rule (owner, 2026-11-07).

#### Scenario: The window drives the export options
- **WHEN** the harness opens Render settings, chooses MKV, H.265, VBR 6 Mbit/s, speed Slow, 10-bit,
  BT.709, FLAC, 25 fps, whole project, and presses Render…
- **THEN** the started job's options carry exactly these settings, the file name ends `.mkv`, the
  estimate reads a size in MB, and a codec the container cannot hold is listed disabled with its
  reason
- **PATH** File → Render… → the Render settings window: Preset "H.264 · MP4 · quality 23" → File:
  Container "MKV" → Video: Codec "H.265" → Rate "VBR", average 6, maximum 9 → Speed "Slow" → Depth
  "10-bit" → Colour "BT.709 limited" → Size and rate: Frame rate "25" → Audio: Codec "FLAC" → File:
  Range "Whole project" — the estimate shows "≈ … MB" → Render… → the save dialog offers
  "kt-<project>-<len>s.mkv"
- **TEST** t2810 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: What the loaded FFmpeg cannot do is shown, not hidden
- **WHEN** the window opens with an availability that lacks `libsvtav1`/`libaom-av1` and Opus
- **THEN** AV1 and Opus are listed disabled with "not in this FFmpeg", and MP4 lists ProRes disabled
  with "not in MP4"
- **TEST** t2811 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: An open window costs nothing at rest
- **WHEN** the window is open on a project with heavy effects and 120 frames pass with the pointer
  still, then 120 with the pointer moving over a static place in the window
- **THEN** no repaint is requested after the first settle, and no export work is started
- **TEST** t2813 (`src/apps/kt-app/src/idle_tests.rs`)

### Requirement: An export writes where its time went
Every export, serial or pipelined, SHALL keep its stage profile whether or not a caller asked for one, and
when it ends SHALL write it into the trace — an `ExportStage` record per stage (busy nanoseconds summed over
the stage's threads, the wall, the frames written, the frame workers) and one for the wall with the timeline
it covered — and SHALL return it as one line (`ExportStats::stages`) that the render window writes into the
telemetry journal. The single-device export SHALL report its upload, encode, device wait and copy-out as the
per-worker devices do.
Why: kkt-26.11.15 N3 — "the final render is slower than the preview in every version"; none of the owner's
traces held an export, and the profile existed only under `kt-cli export --time` on our machine.

#### Scenario: Serial and pipelined exports leave their stages
- **WHEN** a two-second project is exported serially and pipelined, with no profile asked
- **THEN** the trace holds a wall record with 60 frames and the two seconds covered (frame workers 0 only for the serial one), decode and encode records with busy time, the same wall and 60 frames, and the returned line names the frames, the size, the real-time factor and the busy stages
- **TEST** t2792 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: The render window says what the render costs against the preview
The preview SHALL keep the device time of each moment it composed, with its size. The render window SHALL
show the preview's size against the render's and how many times the pixels of a frame the render draws
when they differ, and a render time predicted from the preview's measured device time per pixel over the
range at the render's size, saying how many of the range's frames were measured; with none measured it
SHALL say to play the range once. A replayed kept composite SHALL NOT count as a measurement; a moment the
render-ahead producer composed SHALL.
Why: row 711, review R8 — the owner's preview composes at 640×360 and his render at 1920×1080, nine times
the pixels, and "the final render is slower than the preview" (kkt-26.11.15 item 3) had nothing on screen
to explain it.

#### Scenario: The preview's costs over a range
- **WHEN** moments are recorded at 640×360 with 2 ms and 4 ms of device time, one outside the range, and a
  newer record replaces an older one of the same moment
- **THEN** the range's cost is the newest records' sum over their pixels, the count of moments measured,
  the preview's size; a range with no record has no cost
- **TEST** t2902 (`src/libs/kt-engine/src/preview_cost.rs`)

#### Scenario: The window states the size and the prediction
- **WHEN** the render settings window opens on a 1920×1080 project whose preview runs at 640×360, first
  with no preview cost measured, then with the range's frames measured at 4 ms each
- **THEN** it reads "640×360" against "1920×1080" and "9.0×", first "not measured", then a predicted time
  of frames × 4 ms × 9 and how many frames were measured
- **PATH** File → Render… → the Render settings window: the File group under the size estimate reads
  "Preview 640×360 · render 1920×1080: 9.0× the pixels of a frame" and "Render time: not measured — play the
  range once"; after the range plays, "Render time ≈ … (measured … of … frames)"
- **TEST** t2903 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The producer's moments are measured
- **WHEN** the render-ahead producer composes moments ahead on a sibling of the display's engine
- **THEN** the sibling reports their device time (`last_device_ns` reads its preview frames), so the range
  that played is measured (found-defects 728: read as an offline engine's frames it was 0, and the window
  said "not measured" after the range had played)
- **TEST** t2929 (`src/libs/kt-gpu/tests/render_ahead.rs`)

### Requirement: Every render journals its predicted time against its actual one
A finished render SHALL write one line into the telemetry journal with the time predicted when it started
(with how many frames it was measured from and at what size) and the time it took, or that it had no
prediction.
Why: a prediction from the preview's device time leaves out decode and encode; only the line after each
render tells on the owner's machine how far it is from the truth.

#### Scenario: A render's line
- **WHEN** a scripted render with a prediction of 12 s finishes, and one with no prediction
- **THEN** the journal holds "render time: predicted 12.0 s" with the actual time and their ratio, and
  "render time: no prediction" with the actual time
- **TEST** t2904 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The readback ring follows the device's latency
An export composing on one device SHALL keep as many frames in flight between submission and readback as
the device takes to hand them back, bounded by the ring's bytes: the ring SHALL hold, for a frame size, a
staging budget's worth of frames (at least three), and the compose thread SHALL hand on every frame the
device has finished as soon as it has finished, in submission order, waiting for a frame only while the
ring is full. The frames written SHALL be byte for byte those of a ring of any other depth and of
`execute_into`. The ring's depth and every frame's time from submission to collection SHALL be recorded in
the export's profile and the trace.
Why: kkt-26.11.18 T3, "отрендерил даже самый простой регион - и скорость 1x cur". His trace: the device
worked 1.06 ms a 1080p frame and handed it back 52.7 ms after its submission; a fixed ring of three frames
made the export three frames per 52.7 ms, 0.94× real time of a 60 fps project, with every stage idle.

#### Scenario: The ring holds more than three frames of a 1080p export, and gives the executed bytes
- **WHEN** an export engine is asked its ring depth for 1080p, 4K and 8K frames, and as many 1080p-sized
  demo plans as the 1080p depth are submitted before the first collect, polled until done and collected
- **THEN** the 1080p depth is more than three, a larger frame gets no deeper ring and never fewer than
  three; every submission is accepted, the ring's first frame reports done before it is collected, and
  every collected frame equals the `execute_into` of its plan, byte for byte
- **TEST** t2960 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: The compose thread keeps the device's latency in flight and hands on finished frames at once
- **WHEN** a pipelined export of forty frames runs on an executor whose frames are done a fixed time after
  their submission and whose ring holds twelve; and again on one whose frames are done at once
- **THEN** with the latency the profile's ring records more than three frames in flight at once and the
  frames written equal the serial export's; with no latency at most two frames were ever in flight (each was
  handed on at its next submission); both record forty frames from submission to collection
- **TEST** t2961 (`src/libs/kt-engine/tests/export_pipeline.rs`)
