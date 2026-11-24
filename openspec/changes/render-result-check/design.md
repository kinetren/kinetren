# Design: render-result-check

## Context (orch `5986f08e`)

- `ExportProgress.total_video_frames` follows the render loop (t1070). `total_audio_frames` is the range's samples
  (`export.rs:690`, `r1.to_audio_frames() − r0.to_audio_frames()`); `ExportStats` carries `video_frames` and
  `audio_frames` written and no totals (`:80`).
- His numbers: 412672 is 403 bursts of 1024 exactly, 413364 the range: the last, partial burst (692 samples) was not
  counted. The final block names the written samples alone.
- `renderregion` renders 0.2 s from a third of a frame after 1 s at 24 fps, and checks that the file is an MP4.

## D1. The result knows its totals

`ExportStats { announced_video: u64, announced_audio: u64, … }`, filled where the progress totals are. The sound written
SHALL be the range's samples: the last burst is cut to the range's end, not dropped and not padded (the encoder's own
1024-sample frame may pad inside the codec; `ffprobe`'s duration then differs by less than one AAC frame).

## D2. What the window says

Equal (always, unless there is a defect): as today, one number each. Different: a line in the final block in the
warning style, "Frames written N of M", "Sound written S of T samples", and the same in the journal. The block's
place and the other rows do not move.

## D3. What the file holds

The picture starts at the frame that contains the range's start and goes on to its end, the sound is the range. So for a
grid step `s`: `frames · s ≥ range` and `frames · s < range + 2 s`. The live scenario checks the packets and the sound's
length against these.

## D4. Class walk — "a counter that ends short of or past its own total"

| Counter | Ends equal? | Here |
|---|---|---|
| Render: frames | yes (t1070) | shown pair checked |
| Render: sound samples | unproved | D1, t3998 |
| Render: size, bitrate | an estimate, then the file's real size | no pair |
| Queue of renders and the CLI's `export` line | from the same `ExportStats` | covered by D1 |
| FFmpeg download (bytes) | the length the server announced; absent length shows no fraction | checked: no pair when unknown |
| Analysis jobs (loudness, sync, waveform, thumbnails) | a fraction, no pair of numbers in the window | no pair |
| Prerender and cache bars | coverage, not a counter | — |

## Rejected

- **Bringing the audio row back** — a number that never differs is noise; the line appears when it does.
