# Spec Delta

## ADDED Requirements

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
