# Design

## Context

The export path is `render_dialog.rs` (`RenderSetup`) → `ExportOptions` (kt-engine `export.rs`) →
`MuxConfig` (kt-ffmpeg) → `Muxer::open` (`muxer_impl.rs`, one copy per FFmpeg major). The muxer
hard-codes the container `"mp4"`, AAC, `gop = fps·2`, `max_b_frames = 0`, the encoder's pixel
format, and never touches `color_*`. The cascade (`encoders.rs`) is H.264 only. The reconnaissance
and the verdict per option are in F-EXP-006; this design is the "how".

## Decisions

### D1. One `OutputFormat`, pure, in kt-ffmpeg

`kt_ffmpeg::format::OutputFormat` holds what the muxer needs beyond size, fps, threads and the
existing quality/bitrate/encoder fields: `container`, `video` (codec family), `rate`
(`Quality`/`Vbr`/`Cbr`), `max_bitrate`, `keyint_tenths` (keyframe interval in tenths of a second),
`b_frames`, `speed` (five steps), `depth` (8/10), `matrix` (BT.601/BT.709), `range`
(limited/full), `audio` (codec or none), `title`, `comment`. Its `Default` is today's file:
MP4/H.264/Quality/2 s/0 B/`Fast`(= `veryfast`)/8-bit/BT.601/limited/AAC. `MuxConfig.format` and
`ExportOptions.format` carry it unchanged; `ExportOptions` keeps its existing fields (quality,
bitrates, forced encoder, resolution) so the many call sites and the sibling change in `export.rs`
do not move. `ExportOptions.fps: Option<u32>` overrides the project's rate: the export runs on a
clone of the project whose `settings.fps` is the override, so the frame plan, the totals and the
muxer agree by construction (one value, not two kept in step).

Rejected: putting quality/bitrates into `OutputFormat` too — a cleaner type, but 14 call sites and
the render-speed change editing the same lines at the same time.

### D2. Compatibility is a table, availability is a question to the loaded FFmpeg

`Container::allows_video/allows_audio` is a fixed table (MP4: H.264/H.265/AV1/VP9 + AAC; MOV:
H.264/H.265/ProRes + AAC/PCM; MKV: everything; WebM: VP9/AV1 + Opus). Whether the loaded FFmpeg
HAS an encoder is asked at run time (`avcodec_find_encoder_by_name`), and a device candidate also
needs its device (hw-encode-context D6) — the same rule the H.264 list had. The window asks once
per opening (`start_render`), never per frame. Unavailable entries stay in the lists, disabled,
with the reason on hover-free text next to them (rule: never silently dropped).

### D3. A cascade per codec; the asked depth or rate mode filters it with a reason

`video_encoder_candidates(os, codec)`:
- H.264: unchanged (nvenc, vaapi, qsv, libx264, mpeg4 / Windows: nvenc, amf, qsv, libx264, mf, mpeg4).
- H.265: `hevc_nvenc`, `hevc_vaapi`, `hevc_qsv`, `libx265` (Windows: nvenc, amf, qsv, libx265, mf).
- AV1: `av1_nvenc`, `av1_vaapi`, `av1_qsv`, `libsvtav1`, `libaom-av1` (Windows: nvenc, amf, qsv, svt, aom).
- VP9: `vp9_vaapi`, `vp9_qsv`, `libvpx-vp9` (Windows: `vp9_qsv`, `libvpx-vp9`).
- ProRes: `prores_ks` (profile HQ, `yuv422p10le`).
Each `EncoderSpec` states its 10-bit input (`yuv420p10le` for software, `p010le` for NVENC, none
for VAAPI/QSV/AMF/MF here) and its rate modes. With `depth = 10` or a rate mode the candidate lacks,
the cascade SKIPS it with the reason ("no 10-bit input", "no CBR") into `OpenReport.skipped` — the
render log already prints every skip. A forced encoder that lacks it errs naming the reason.
Whether libx264 of this build takes 10-bit is the real open's answer, not a table's.

### D4. Rate control maps onto the encoder's own words

Quality: the existing `quality_options` (CRF/CQ/QP/global_quality/MF quality) plus `crf` for
x265/svt/aom/vpx (with `b=0` for aom/vpx — constant quality). VBR: `bit_rate` + `rc_max_rate`
(the maximum, default 1.5× the average) + `rc_buffer_size = 2 × max`. CBR: `bit_rate =
rc_max_rate = rc_min_rate`, `rc_buffer_size = bitrate`, plus the encoder's own switch (`rc=cbr`
NVENC, `rc_mode=CBR` VAAPI, `rc=cbr` AMF, `rate_control=cbr` MF, `nal-hrd=cbr` x264). ProRes has
no rate control: the window disables the row and says why. Two-pass is rejected (F-EXP-006):
it needs a stats FILE (invariant 9) and a second full render.

### D5. Speed is five steps mapped per encoder

`Speed::{Fastest, Fast, Balanced, Slow, Slowest}`: x264/x265 `ultrafast/veryfast/medium/slow/
veryslow`, NVENC `p1/p4/p5/p6/p7`, QSV `veryfast/faster/medium/slow/veryslow`, SVT-AV1 `12/10/8/6/4`,
libaom `cpu-used 8/6/4/2/1`, libvpx `deadline realtime cpu-used 8 / good 5 / good 3 / good 1 /
best`, AMF `speed/speed/balanced/quality/quality`; VAAPI/MF/ProRes have none (shown as such). `Fast`
is the default because `veryfast`/`p4` IS today's file.

### D6. The colour the file says is the colour the conversion made

The RGB→YUV conversion (`VideoConverter`) now calls `sws_setColorspaceDetails` with the chosen
matrix and range, and the encoder context gets `colorspace`, `color_range`, `color_primaries =
BT.709`, `color_trc = BT.709`. The default stays BT.601 limited — exactly what `sws` did by default
and what the preview shows: the decode side converts every source with the same BT.601 default
(found-defects row: sources tagged BT.709 are decoded with BT.601; a separate change), so a file
tagged BT.601 plays as the preview looked. BT.709 is offered for delivery specs that demand it.
Full range sets `color_range = JPEG` and full-range coefficients on the conversion.

### D7. Audio codecs feed their own sample format

The muxer keeps the interleaved f32 FIFO (invariant 1 up to here) and converts at the frame:
AAC → FLTP, Opus (`libopus`) → FLT, PCM 16 → S16, PCM 24 → S32 with `bits_per_raw_sample = 24`,
FLAC → S32/24. Conversion is clamp·scale·round in Rust (deterministic, no libm). A frame size of 0
(PCM) uses 1024. "None" opens no audio stream; the engine still renders the audio pass (a cost the
range shows; skipping it would touch the pipeline the render-speed change is changing).

### D8. Container details

MP4 and MOV get `+faststart`. H.265 in MP4/MOV is tagged `hvc1` (Apple players refuse `hev1`).
Metadata goes into the format context's dictionary (`title`, `comment`); no `creation_time` is
written, so the bytes stay deterministic (invariant 11). The file name keeps the owner's template;
only the extension follows the container (`.mp4/.mov/.mkv/.webm`).

### D9. One text form for kt-cli and presets

`kt_engine::export_settings` turns `ExportOptions` (format, quality, bitrates, encoder, size, fps)
into ordered `key=value` pairs and back; an unknown key or a bad value is an error naming it.
kt-cli `export` takes `--set key=value` (repeatable) and short flags for the common keys. A preset
in `render-presets.json` stores the same pairs: a record with a key this build does not know is
SHOWN as skipped and kept verbatim on save (user-store rule: an old build does not eat a new one's
work). kt-cli keeps ignoring the region (owner's decision, export spec).

### D10. The window

Groups (R.66 form): Preset (combo + Save as + Delete), Video (codec, encoder, rate, quality or
bitrate+max, speed, keyframes, B-frames, depth, colour), Size and rate (size, custom W×H, fps),
Audio (codec, bitrate), File (container, range, title, comment, estimated size), then the existing
warnings; the buttons stay outside the scroll. Numbers go through the shared rows. The last settings
are saved when a render starts and loaded when the window first opens in a session.

## Risks

- Hardware candidates for H.265/AV1/VP9 cannot be proven on this machine (no NVENC; VAAPI is
  proven by `make hw-test` only for H.264). They pass through the same throwaway-process trial
  (D10 of hw-encode-context) and the same skip rules; the software candidates are tested.
- `libsvtav1` determinism across runs at `threads auto` is not guaranteed by its vendor; the
  determinism test covers libx264, libx265, libvpx-vp9 and prores_ks, and names AV1 as not covered.

## The class walk

The mechanism behind "very scant settings": a property of the written file was FIXED in code and
either not shown or shown as a constant that could lie once the file changed. Every place that
states or decides a property of the output was walked:

- the settings window — every row now a choice or an honest constant ("48 kHz stereo", invariant 1);
- the progress window's Output rows — "Audio: AAC …" and "Container: MP4 (+faststart)" were
  literals; now read from the job's format (`audio_row`, `mux_row`); its title "Render to MP4" now
  names the container; the finalize phase no longer says "(faststart)";
- the File menu, the toolbar hint and the Guide's lesson row said "Render to MP4" — now "Render…";
- the file name's extension and the save dialog's filter followed nothing — now the container;
- `kt-cli export` knew only an encoder — now the same settings; `kt-cli ffmpeg --probe-encoders`
  still probes the H.264 hardware candidates only (left as is: it is a driver report, not a render);
- the throwaway hardware trial (`hw_trial`) opened every name inside the H.264 cascade — now inside
  the codec family the name belongs to (`encoders::codec_of`);
- the colour: the export wrote no tags (found-defects 693, fixed); the DECODE converts every source
  with swscale's BT.601 default whatever its tag (found-defects 694, open — a change of its own);
- determinism: Matroska drew a random segment id per file (found here by t2805, fixed with the
  bit-exact flag only for Matroska/WebM);
- the render log and the telemetry journal already print the open report's options and skips, so the
  new skip reasons ("no 10-bit input", "no CBR") reach them without a change.
