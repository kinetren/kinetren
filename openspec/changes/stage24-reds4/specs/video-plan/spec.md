## ADDED Requirements

### Requirement: A decode step no slot consumes comes back to the canvas
A layer read at a decode step SHALL reach the composite at the canvas's size at every moment, also at one
whose resolved chain has no placement that consumes the step: a zoom curve at 1, slots that are all identity.
The slots in front of a stream stage SHALL ask no decode step of its layer.

Why: found-defects 843 — such a layer was never drawn: the preview stood at Loading, an export could not
write the frame, a debug build panicked.

#### Scenario: A zoom animation stands at 1
- **PATH** A 4K event, the preview at 1/4 → Geometry › Zoom with keys 1, 1 and 4 → the playhead before the second key → the picture is there → play: the zoom begins with no jump
- **WHEN** a curve of a slot, of the Geometry node, under a mask, a level or a track's level stands at 1
- **THEN** the frame is complete at every moment
- **TEST** t3262 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A Datamosh behind a zoom
- **WHEN** a zooming Geometry stands in front of a stream stage, or after it
- **THEN** the frame is complete
- **TEST** t3263 (`src/libs/kt-engine/tests/stream_stage.rs`)
