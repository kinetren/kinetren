## ADDED Requirements

### Requirement: Starvation crosses a reel's boundary
A missing frame inside a reel SHALL make the outer composite incomplete, and the warm composite SHALL
be complete and equal to the export's.
Why: otherwise the player takes the frame for ready and never asks again.

#### Scenario: A child's frame is missing
- **WHEN** a reel's composite runs on a cold cache, then on a warm one and on an export's own cache
- **THEN** the first is incomplete, the others complete and byte-equal
- **TEST** t441 (`src/libs/kt-engine/tests/nest_render.rs`)
