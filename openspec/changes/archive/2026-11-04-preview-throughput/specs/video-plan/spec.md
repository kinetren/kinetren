## ADDED Requirements

### Requirement: The GPU preview's picture stays on the device
With a GPU executor the preview SHALL compose through `VideoGpuExec::present`: the passes of
`execute_into`, then a display pass into a texture the UI draws, with no wait for the device and
no copy of the frame to the CPU. The display target SHALL hold the bytes the readback path shows,
premultiplied as `Color32::from_rgba_unmultiplied` does. An incomplete frame SHALL leave the target
as it is, unless nothing has been shown yet. Without an executor, or for a plan it refuses, the
preview SHALL take the bytes path. Export SHALL keep its readback.
Why: at 1080p the UI thread spent about 33 ms of a 40 ms frame uploading, waiting for the device
and copying the frame out, before converting it and uploading it a second time (`kt-cli preview`,
llvmpipe; preview-throughput, 2026-11-04). The p95 stall per UI frame went from 36–55 ms to 7–11 ms.

#### Scenario: The picture does not change when the readback goes
- **WHEN** half-transparent events are presented and executed at the same moments, across a
  transition and over effects
- **THEN** the display target equals the readback's bytes premultiplied by egui's rule
- **TEST** t1525 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: An incomplete frame holds the last full one
- **WHEN** a starved frame is presented after a full one
- **THEN** the display target is unchanged, and with nothing shown yet the frame is drawn
- **TEST** t1526 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The glow and a new size
- **WHEN** a frame is presented with the glow, and then at another preview size
- **THEN** the glow holds the pixels of the CPU loop's 32×18 picks, and the new size is a new
  texture with a new generation, while the same size keeps its generation
- **TEST** t1527 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The program draws from the device
- **PATH** open a project with a flat clip → the light frame shows → click the ruler over a clip with
  Edges → the frame goes black, and the program reports the picture was drawn on the device
- **TEST** live scenario `src/scripts/live/gpufx.py`

### Requirement: The CPU preview composes ahead of the display
Without a GPU executor, a playing preview SHALL compose on worker threads ahead of the display, with
the same call as the pipelined export, and the display SHALL take a finished frame without composing
it. A frame SHALL be served only for the project version, content version and quality it was
composed under. Workers SHALL read the cache alone and SHALL compose only frames whose layers it
holds. Nothing SHALL be composed ahead while the transport stands, and a project with a stream
stage SHALL keep the synchronous path. While the workers serve, a missed frame SHALL hold the picture
and SHALL order no decode; the hold SHALL end, and the frame be composed, when no worker holds that
frame and the display has stood on its tick for three frames, and it SHALL NOT start at a rate of
zero or below.
Why: the CPU executor is one frame at a time on the UI thread, p95 381 ms at 1080p on the showcase,
8–9 V-fps (preview-throughput, 2026-11-04). Ahead: 22–25 V-fps with the UI's worst stall under 25 ms.
A Playback request for the frame on screen, behind the look-ahead, seeked the decoder back and
silenced the audio on the same worker: 38,026 frames of underrun in 31 s.

#### Scenario: A frame composed ahead is the same frame
- **WHEN** a range is played on the CPU path and the display takes frames from the workers
- **THEN** every frame shown equals the plan's synchronous composite of its moment
- **TEST** t1530 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: Nothing runs in a pause
- **WHEN** a paused frame is composed, and later playback stops
- **THEN** no worker starts for the paused frame, and after the composites under way finish nothing
  more is composed
- **TEST** t1531 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A frame of another project state is not served
- **WHEN** a frame finished under one project, content or quality state is asked under another
- **THEN** it is not served, and the buffer on screen is untouched
- **TEST** t1532 (`src/libs/kt-engine/src/compose_ahead.rs`)

#### Scenario: A stream stage stays synchronous
- **WHEN** a project with a datamosh slot plays on the CPU path
- **THEN** its frames are composed and none of them ahead
- **TEST** t1533 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: The display orders no decode while the workers serve
- **WHEN** the workers serve and the display asks a frame past the look-ahead
- **THEN** the picture holds, and the decoder is never asked for that frame
- **TEST** t1534 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A held picture does not freeze
- **WHEN** the workers serve, and the clock stands, or the rate is reversed or zero
- **THEN** a frame asked every UI frame arrives
- **TEST** t1536 (`src/libs/kt-engine/tests/preview_throughput.rs`)
