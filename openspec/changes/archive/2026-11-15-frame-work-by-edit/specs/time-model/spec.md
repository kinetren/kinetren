## ADDED Requirements

### Requirement: The walk meter counts per-frame work
`kt_time::cost` SHALL carry the rows `FrameClip` (an event a per-frame question looked at),
`ChainClone` (a chain copied for a frame's plan) and `ParamByName` (a parameter set by name on a
per-frame path), appended after the existing rows; like the others they count, never time.

#### Scenario: The rows feed the frame guards
- **WHEN** the frame guards run
- **THEN** they read these rows, and a scan form of the index turns them red
- **TEST** t2660 (`src/libs/kt-engine/tests/frame_cost.rs`)
