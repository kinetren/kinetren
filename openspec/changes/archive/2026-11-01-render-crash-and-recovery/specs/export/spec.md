## ADDED Requirements

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
