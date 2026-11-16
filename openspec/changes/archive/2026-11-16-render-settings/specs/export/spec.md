## ADDED Requirements

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
primaries and transfer. The default SHALL be BT.601 limited — the conversion every export has used.
Why: the files carried no tags at all; a player guessed BT.709 for HD and showed different colours
than the preview (F-EXP-006 trick).

#### Scenario: A saturated colour comes back by the tag
- **WHEN** a flat saturated red frame is exported with BT.601 limited, BT.709 limited and BT.601 full
- **THEN** the reader reports the three tag pairs (smpte170m/bt709 matrix, limited/full range,
  BT.709 primaries and transfer); the default file reads back as written within 6 codes, and the
  BT.709 file, read back by the BT.601 decode, differs from it by more than 5 codes — the
  conversion changed, not only the label
- **TEST** t2806 (`src/libs/kt-engine/tests/render_settings.rs`)

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
