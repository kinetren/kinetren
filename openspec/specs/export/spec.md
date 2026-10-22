# Export and headless render

## Purpose

Offline run of the same engine as the preview: in-process muxing to mp4 through the system
FFmpeg, golden references for raw streams, the headless `kt-cli` tools (render/export/probe),
and cross-render against another editor as external conformance.

## Requirements

### Requirement: Export is fully in-process, the encoder is chosen by opening it
Muxing and encoding SHALL go through libavformat/libavcodec of the SYSTEM FFmpeg, loaded
at runtime (capability `media-index`): no temporary files, no external `ffmpeg` binary,
no mkfifo. The video encoder SHALL be chosen by trying the candidates
`h264_nvenc → h264_vaapi → h264_qsv → libx264 → mpeg4` with a REAL open check
(`avcodec_open2`) in the same process that writes the file; an explicitly requested encoder MUST
either open or produce an export error — there is no silent substitution. The name of the actual
encoder SHALL be reported in the export statistics. Frames MUST NOT be lost to encoder
backpressure: on EAGAIN the send is retried.
Why: the presence of an encoder in a build does not mean it will work (nvenc without libcuda fails),
and we have no cached "verdict of usability", so there is nothing that could diverge from reality.

#### Scenario: Auto selection and forced encoder
- **WHEN** export starts without an encoder specified / with `libx264` / with a nonexistent name
- **THEN** auto picks the first candidate that opens; `libx264` is chosen exactly, and the file is read by our own decoder; an unknown name is an explicit error, and no file is left
- **TEST** t25 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: All frames in the file
- **WHEN** 60 frames are exported at 30 fps
- **THEN** the engine delivered 60 frames AND, in the file read by our own decoder across all grid keys, no fewer than 60 frames are decoded
- **TEST** t33 (`src/libs/kt-engine/tests/export_options.rs`), t10 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`) — our own decoder serves as the verifier because its seek is sample-accurate

#### Scenario: Cancel and error leave no partial file
- **WHEN** export is cancelled or fails in the middle of the video or audio phase
- **THEN** the muxer is closed, the partial file is deleted, and the result is `Cancelled` or an error
- **TEST** t25 family (`src/libs/kt-engine/tests/export_options.rs`) — a forced encoder error leaves no file; the cancel path is `export_mp4_with_backends` (`src/libs/kt-engine/src/export.rs`)

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
48 kHz/2ch and RGBA frames to the callbacks BEFORE the muxer; their hashes MUST be byte-for-byte equal
between runs and (on linux/x86_64) to the committed `GOLDEN_AUDIO`/`GOLDEN_VIDEO`; the title scene —
to its own `GOLDEN_TITLE_VIDEO`. A reference changes ONLY by a deliberate decision with the reason
and the previous value recorded next to the constant (for the title — also in `FONT-MANIFEST.txt`); in
the reference projects there is no net zoom and the native size of the synthetics is smaller than Quality, so the decode scale
there is ≡ 1, and new mechanisms MUST leave the reference in place.
Why: the reference is the only guard of determinism (invariant 11) at once for the audio mix,
warp, transitions, composite and SIMD kernels; the bytes of the output .mp4 will never be
identical for two users (the encoder is a property of the machine), so the stream is hashed BEFORE the
encoder.

#### Scenario: Determinism and the reference
- **WHEN** the golden project is rendered twice
- **THEN** the audio and video hashes match between runs (120 000 audio frames, 75 video frames) and equal the committed constants on linux/x86_64
- **TEST** t11_streams_are_deterministic_across_runs, t11_streams_match_committed_golden (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: The title reference is separate and does not move
- **WHEN** the title scene is rendered twice / the title parameters have moved to an event
- **THEN** the hash is stable and equals `GOLDEN_TITLE_VIDEO` (gated linux/x86_64); moving the values did not shift the reference
- **TEST** t485_title_export_is_deterministic, t485_title_matches_committed_golden, t662 (`src/libs/kt-engine/tests/title_golden.rs`)

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
