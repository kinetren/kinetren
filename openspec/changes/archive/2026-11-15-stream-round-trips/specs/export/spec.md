## ADDED Requirements

### Requirement: A stream stage takes the pipelined export
A project with a stream stage SHALL export through the pipeline: the range's history pre-rolled on the
decode thread, the plan of each frame built in frame order on the compose thread and handed to a worker;
the frames SHALL be the serial export's.
Why: found-defects 490 — a render with a Datamosh ran serially, the device and cores idle between frames.

#### Scenario: The pipelined export walks the stream in order
- **WHEN** a cut mosh is exported pipelined on one device and on three workers, over chunks of four frames, from a range before the stream and from one inside its history
- **THEN** no frame loses a layer and the frames equal the serial export's
- **TEST** t2707 (`src/libs/kt-engine/tests/stream_stage.rs`)
