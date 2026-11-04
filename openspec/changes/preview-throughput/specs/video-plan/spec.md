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
llvmpipe; preview-throughput, 2026-11-04). The p95 stall per UI frame went from 36 ms to 7–8 ms.

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
