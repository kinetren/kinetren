## ADDED Requirements

### Requirement: Reel coverage costs the placements' windows
`nest_coverage` SHALL ask a reel only for the inner events its placement's window meets, subtract
their cold parts from the window in one sorted pass, and answer the placements of one release over
one window and decode quality once per call. Inside a build a reel SHALL be found by id through an
index (`kt_model::with_sequence_index`), never by a scan of the reels per event.

#### Scenario: Window, not reel; once per window
- **WHEN** a reel of 4 and of 64 events is placed once over its first second, and a reel of 4 events is placed 16 times over the same second
- **THEN** the coverage walks the same count of events in all three
- **TEST** t2663 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: One-pass subtraction
- **WHEN** random sorted runs have random holes subtracted by the one-pass form and by the former one-hole-at-a-time form
- **THEN** the results are equal
- **TEST** t2672 (`src/libs/kt-engine/src/video.rs`)

#### Scenario: Reels by index in a build
- **WHEN** a video build of 4 and of 64 placed reels is counted
- **THEN** the reel lookups grow at most linearly (≤ 32× for 16× the reels)
- **TEST** t2664 (`src/libs/kt-engine/tests/frame_cost.rs`)
