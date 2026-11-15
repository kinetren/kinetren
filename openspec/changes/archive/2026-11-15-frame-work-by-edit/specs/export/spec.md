## ADDED Requirements

### Requirement: The export's serial choice builds nothing
Whether an export keeps the serial composite for a stream stage SHALL be asked of the project's
slots (`project_may_stream`: events of every reel and morpher patches), never of a renderer built
only to answer; it SHALL say yes wherever a build would.

#### Scenario: Slots, not a build
- **WHEN** a project without a stream slot, with one on an event and with one inside a reel is asked
- **THEN** no renderer is built and the answer equals the build's `has_stream_stages`
- **TEST** t2668 (`src/libs/kt-engine/tests/frame_cost.rs`)
