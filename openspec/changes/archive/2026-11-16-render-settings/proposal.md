# Proposal

## Why

The owner (`kkt-26.11.15` item 5, `docs/ktprompts/kkt-26.11.15-index.md` N5): "очень скудные
настройки финального рендера прям очень" — the final render's settings are very scant. The render
window offers an H.264 encoder, CRF 14–32 or a bitrate, an AAC bitrate and a frame size. The
container is MP4 and nothing else, the encoder speed is `veryfast` for ever, the keyframe interval
is two seconds, there are no B-frames, the picture is 8-bit 4:2:0, the file carries no colour tags at
all, the frame rate is the project's, the range is not shown, nothing is remembered between launches
and kt-cli knows only an encoder name. The reconnaissance note F-EXP-006
(`docs/features/export/presets/render-settings.md`) compares what Vegas, Premiere/Media Encoder,
Resolve, FCP/Compressor, After Effects and HandBrake offer and decides, option by option, what we
take and what we reject.

## What Changes

- An output FORMAT travels from the window (and kt-cli) to the muxer: container (MP4, MOV, MKV,
  WebM), video codec (H.264, H.265, AV1, VP9, ProRes 422 HQ), rate control (Quality, VBR with a
  maximum, CBR), keyframe interval, B-frames, encoder speed (five steps mapped onto each encoder's
  presets), bit depth 8/10 (the profile follows), colour matrix BT.601/BT.709 and range
  limited/full (written as tags AND used by the RGB→YUV conversion), audio codec (AAC, PCM 16,
  PCM 24, FLAC, Opus, none), title and comment metadata, a frame-rate override and a custom size.
- Each video codec has its own hardware-first cascade per OS opened with a real `avcodec_open2`
  (invariants 9, 10). A candidate that cannot do the asked depth or rate mode is SKIPPED with the
  reason in the open report (never silently); a forced one errs naming it. The window shows an
  option the loaded FFmpeg cannot open as unavailable with the reason.
- The default stays byte-compatible in substance: MP4, H.264, Quality 23, `veryfast`, two-second
  GOP, no B-frames, BT.601 limited — the conversion the export has always done, now TAGGED.
- One text form of the settings (`key=value`) is shared by kt-cli flags and kt-app presets.
- Named render presets and the last settings live in the user store (`render-presets.json`),
  between projects; factory presets are named by what they are, not by a platform.
- The render window shows the range (whole project / region) and an estimated size where the rate
  is known.
- Rejected with reasons (F-EXP-006): two-pass, a manual level, tune, 4:4:4, a scaling filter,
  interlacing, other sample rates and channel layouts, a manual VBV buffer.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `export`: the output format reaches the file (new requirements: the format, the per-codec
  cascade and its unavailable options, the colour tags, the kt-cli flags, the presets, the window).

## Impact

- `src/libs/kt-ffmpeg`: `format.rs` (new, pure), `encoders.rs` (per-codec cascades and options),
  `muxer_impl.rs` (container, codec, rate, GOP, B-frames, depth, colour, audio codecs, metadata),
  `api.rs` (`sws_setColorspaceDetails`, `sws_getCoefficients`, `avformat_open_input` reader for
  the output probe).
- `src/libs/kt-engine`: `ExportOptions.format` and `.fps`; `export_settings.rs` (the text form).
- `src/apps/kt-cli`: `export` flags.
- `src/apps/kt-app`: `render_dialog.rs`, `user_store.rs`, translation catalogs.
- No project-format change: render settings live between projects (user store), not in `.kt`; the
  OTIO door is not touched.
