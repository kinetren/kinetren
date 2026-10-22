# Spec Delta

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
An explicitly requested encoder MUST either open, or give an export error — there is no
silent substitution. The name of the actual encoder SHALL be reported in the export statistics. Frames MUST NOT
be lost on encoder backpressure: on EAGAIN the send is repeated.
Why: the presence of an encoder in the build does not mean it will work (nvenc without libcuda crashes),
and we have no cached "verdict of fitness", so there is nothing that can diverge from reality.
On Windows there is no VAAPI; AMD and Intel without their own encoders in the list fell back to `mpeg4`
if the FFmpeg build has no libx264 (LGPL builds). `h264_mf` (Media Foundation) is present on any
Windows, but its quality is worse than libx264's, so it comes after libx264. QSV did not
open with YUV420P at all.

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
- **TEST** t857 (`src/libs/kt-ffmpeg/tests/encoders.rs`)

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
- **WHEN** the reference tables (export, title, video FX, FX thumbnails, merging of geometry series) are read by the guard
- **THEN** each platform from the shipped list (linux/x86_64, windows/x86_64) has an entry in each table, and each entry has a provenance line (machine, run); on a platform without an entry the comparison test is skipped with a named reason, rather than passing silently
- **TEST** t858 (`src/libs/kt-engine/tests/golden_export.rs`)
