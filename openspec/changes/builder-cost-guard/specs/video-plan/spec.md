## ADDED Requirements

### Requirement: A video build's work grows linearly with the project
`VideoRenderer::new` (and `gen_sources`, which walks the same instance tree) SHALL do work linear in
the project, by the same rule and the same counting guard as the audio build: transition ends and
their engine events by index, a reel's revision once per build (`revisions_scope`), the profile of a
speed curve once per event.

#### Scenario: Events, transitions and placements
- **WHEN** the generated scenes of the audio build's guard (events with transitions, a long signed speed curve, placements, nested placements, a cutting layer, linked curves) are built ×1, ×4, ×16
- **THEN** the video build and `gen_sources` grow at most 8× and 32× in walked elements and in allocated bytes
- **TEST** t2062 (`src/libs/kt-engine/tests/build_cost.rs`)
