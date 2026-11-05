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
(cold and warm post-FX cache), with a generator, and with a title; on GPU, by the same
GPU executor. Source facts (native, procedural) export SHALL query open sources the same way as the
player, and the list of generator sources SHALL be computed by ONE code path (`gen_sources`) for the player, export and kt-cli.
Why: two composition codes ("PlaybackPipeline versus ProjectRenderer") were a defect n
of the old engine: the preview showed one thing, and the file contained another.

#### Scenario: Export frame equals preview
- **WHEN** frame No. 17 of the export stream and the preview composite of the same tick at the same Quality are taken
- **THEN** the bytes are equal
- **TEST** t11_preview_equals_export_frame (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: With FX chain, cold and warm
- **WHEN** on video clips the chain is Mirror → Glow → VHS; the preview is composited twice (the second time from the post-FX memoization)
- **THEN** both composites are byte-for-byte equal to the export frame
- **TEST** t67 (`src/libs/kt-engine/tests/golden_export.rs`), t66c (`src/libs/kt-engine/tests/postfx_cache.rs`) — the export composite does NOT memoize post-FX, but the bytes are the same

#### Scenario: Generator and title in the render path
- **WHEN** the scene contains a solid generator / a title (capability `generators`)
- **THEN** export is deterministic between runs, the frame in the generator's zone has the set color, and preview == export byte-for-byte
- **TEST** t479 (`src/libs/kt-engine/tests/golden_export.rs`), t485_title_preview_equals_export (`src/libs/kt-engine/tests/title_golden.rs`)

#### Scenario: On GPU, by the same executor
- **WHEN** an adapter is present; preview and export are executed by the GPU backend, cold and warm
- **THEN** frames are equal byte-for-byte (without an adapter — skip; `KT_REQUIRE_GPU=1` requires it)
- **TEST** t184 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: A started export completes on the backend it began with; without GPU everything renders
The export backend (GPU executor or CPU) SHALL be passed explicitly by the product (kt-app/kt-cli);
test constructors use CPU. A GPU error IN THE MIDDLE of an export MUST be an export error, not a
silent fallback to CPU: one file does not mix backends. The CPU path SHALL be complete —
3D effects (Pan3d, cameras) are computed on CPU too; there is no "needs GPU" refusal; export without an adapter
is legal and merely slower. In kt-cli, `KT_VIDEO_GPU=0` forces CPU.
Why: mixing backends inside one file would produce a visible seam within the GPU↔CPU tolerances, and a
"3D only on GPU" refusal turned a missing driver into the impossibility of rendering the project.

#### Scenario: Export without GPU carries 3D
- **WHEN** a project with Pan3d is rendered with `ExportBackends::default()` (CPU)
- **THEN** the export passes, the frame differs from the frame without the effect, and with a live adapter the CPU and GPU agree within the tolerance `FrameTol::GEOM`
- **TEST** t301 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: Golden references for raw streams
The raw stream functions (`render_raw_audio`/`render_raw_video`) SHALL deliver audio f32
48 kHz/2ch and RGBA frames to the callbacks BEFORE the muxer; their hashes MUST be byte-for-byte equal between
runs and to the committed `GOLDEN_AUDIO`/`GOLDEN_VIDEO` of EVERY
shipped platform (a table by `(os, arch)`: linux/x86_64, windows/x86_64); the title
scene — to its own `GOLDEN_TITLE_VIDEO` from the same table. Hash checks that today are not
limited by platform and differ between OSes only through libm (video FX, FX thumbnails, merging of
geometry series) SHALL use the same table. On a platform that is not in the table,
the comparison with the constant is skipped with a named reason, while the check between runs
remains. The reference changes ONLY by a conscious decision with a record of the reason and the previous value
next to the constant (for the title — also in `FONT-MANIFEST.txt`); in the reference projects there is no net
zoom and the native synthetic size is less than Quality, so the decode scale step there is ≡ 1, and new
mechanisms MUST leave the reference in place. The constant of a new platform is entered after the
first green run on it, with a record of which machine and which run it was taken on.
Why: the reference is the single guard of determinism (invariant 11) at once for the audio mix,
warp, transitions, composite and SIMD kernels; the bytes of the output .mp4 will never be
identical for two users (the encoder is a property of the machine), so the stream is hashed BEFORE the
encoder. Transcendental functions (`sin`, `exp`, `powf`) are taken from the platform's libm
(glibc on Linux, the mingw CRT on Windows) and may diverge in the last bit: one constant
for all OSes would keep Windows red forever, and its absence would be unguarded.

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
Export SHALL decode frames in batches via span reads (by the same path as the preview) and
composite from the cache; the batch size MUST be computed from the cache budget (no more than half),
the frame size at the LARGEST decode quality (decode scale step S gives S² memory), and the number
of simultaneous video layers (a blending layer counts as two frames; a pair inside a reel is counted).
The export composite MUST NOT memoize post-FX frames. The finished export frame MUST NOT
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
of the finished mix, feeding the meter with the FINISHED chunk buffer — not a single extra decode or
pass. The master gain MUST NOT take part in the measurement and MUST NOT exist in the project
model: it is a monitoring loudness that lives only in the player. There is no "normalize" button —
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
The default name SHALL be `kt-<project>-<region>-<length>s.mp4`, where the region is the name of the
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
`export` (mp4, the third argument is a forced encoder). Writing a project SHALL go through the same
atomic writer as the GUI; an existing file MUST NOT be overwritten without `--force`.
`demo` by default SHALL refer to synthetic sources (`synth:`), so that
`demo && render` works without a single media file. Relative media paths SHALL be resolved
exactly once (when the project is loaded), and `.ktproj` in `render/export` — a refusal with a hint to
`convert`. Export opens the files with the project's sidecar index.
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
- **TEST** t307 (`src/apps/kt-cli/tests/cli.rs`), t34 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`) — opening with the sidecar index does not rescan the file again

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
the same project gives the same audio bytes and CPU pixels on Linux and Windows and on any CPU.
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
(GPU, with frames in flight between submit and readback) or on frame-parallel workers (CPU, each
with its own scratch), the RGBA→encoder conversion on frame workers, the encoder and the muxer on
one writer, and the audio on its own thread. The stages SHALL be joined by bounded queues that
keep frame order, and the encoder SHALL receive exactly the frames and samples, in exactly the
order, of the serial export (`ExportOptions.serial`, the former one-thread export kept as the
reference); the writer SHALL interleave the audio by a fixed rule (after video frame `i`, the
audio up to that frame's end), so the call order into the muxer and the progress events do not
depend on timing. A frame being composed SHALL never be evicted before its composite (decode
waits while the unwritten chunks' frames fill the look-ahead budget). A cancel or an error in
any stage SHALL stop every stage promptly and return the cause. The video encoder SHALL be opened
with its own threads (`threads=auto`; the serial reference `1`), named in the open report. On a
software rasterizer (llvmpipe, lavapipe) the export SHALL compose on the CPU (`KT_VIDEO_GPU=1`
takes the adapter anyway). `kt-cli export --time` SHALL print the busy time of every stage and
the wall time; `--serial` runs the reference.
Why: the owner (2026-11-03, kkt-26.11.03 C1): "our final render is VERY slow, slower even than the
preview; it seems not parallel but strongly sequential". It was: one thread did decode →
composite → readback (a full wait per frame) → conversion → encode, libx264 encoded on one
thread, and the audio pass started after the last video frame. The showcase export took 324 s (llvmpipe)
and 861 s (CPU) for 104 s of 1080p25, and now takes 96 s.

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
`src/libs/kt-engine/src/export.rs`), but today only the final stats name it, so the progress
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
bitrate), mux (container and `+faststart`), backend (GPU with the adapter name, or CPU with the
reason a GPU was not used) and the output file path; **Run** — phase, progress as done/total
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
and Cancel requests and the result. The same lines SHALL be written to `<data_dir>/render.log`; the
previous run's file SHALL be moved to `render.log.1` when a new render starts (two files, no
numbered fan), and the window SHALL show the log path. Writing the log SHALL never fail the render:
an unwritable file gives one journal line and the window says the log is only in the window.
Why: v1 had an 800-line log panel and wrote a log file on error
(`projectrenderer.cpp`, `render-error-<UTC>.log`); today a failed render leaves one error line in
the window and nothing on disk, and the GPU-backend failure goes to `eprintln!`, which a desktop
start never shows (`.desktop` has `Terminal=false`). Two files mirror the journal's rotation
(`journal.rs`: one step, no fan of numbers).

#### Scenario: The log is bounded, rotated and named
- **WHEN** the harness runs a scripted job that emits 1000 log lines and an error, with the data
  directory in a temp folder that already holds a `render.log` of a previous run; separately, the
  data directory is read-only
- **THEN** the window's log holds the last 800 lines, each starting with an elapsed stamp, and is
  open after the error; `render.log` holds all lines of this run, `render.log.1` holds the previous
  file's bytes, and the window shows the path of `render.log`; with the read-only directory the
  render result is unchanged, one journal line is written and the window says the log is only in
  the window; every new text of this change exists in the five catalogs with its placeholders and
  its glyphs are in the program's fonts
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
