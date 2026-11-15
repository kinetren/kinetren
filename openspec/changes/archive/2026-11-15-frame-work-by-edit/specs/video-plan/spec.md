## ADDED Requirements

### Requirement: A frame's work grows with the events active at it
Every per-frame question of a video renderer — the plan, the order (`layers_at`, `wanted_frames`,
`absent_layers`), a stream stage's advance, want, bytes and checkpoints — SHALL find the events active
at the moment through the renderer's time index (`LayerIndex`), laid out once per build in the
composite order; a dissolve's partner and a track stage's events, revision and masks SHALL be laid out
once per build too. The index SHALL answer exactly the former scan, in its order. A stream stage's
second input at a position SHALL be found by halving, not by a scan of the lane.

#### Scenario: Far events do not tax a frame
- **WHEN** a scene with a dissolve, an animated layer, an animated mask and a camera is planned over 50 frames with 20, 80 and 320 one-second events far after it
- **THEN** the events looked at (`Cost::FrameClip`) are the same count in all three
- **TEST** t2660 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: The order costs its window
- **WHEN** `wanted_frames` of the scene's first two seconds is asked with 20, 80 and 320 far events
- **THEN** the count is the same in all three
- **TEST** t2661 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: The index is the scan
- **WHEN** the events active at every 1/50 s of a scene with overlaps, transition handles and an event under everything are asked of the index and of the former scan
- **THEN** both give the same events in the same order
- **TEST** t2662 (`src/libs/kt-engine/tests/frame_cost.rs`)
