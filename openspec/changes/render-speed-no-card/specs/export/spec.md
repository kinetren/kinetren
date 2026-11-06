## ADDED Requirements

### Requirement: A render goes on past the end of a cut file
A render of an event longer than its file holds SHALL give every frame, the frames past the end with
the file's last picture, and lose no layer.
Why: the render looked stalled and nothing named the cause.

#### Scenario: An event over the cut
- **WHEN** a 20 s event reads a 12 s file cut to about 6 s
- **THEN** 600 frames are written, no layer is lost, the decoder reopens at most once and decodes
  under 300 pictures
- **TEST** t1763 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A pipelined chunk is counted by what its frames ask
The chunks of the pipelined export SHALL be frames in a row up to half a budget of what those frames
ask, and one frame when it alone asks more. The serial loop keeps its measure.
Why: the project's largest size made a chunk of one frame on the showcase: the sources advanced in step
and nothing ran in parallel within a source.

#### Scenario: A stretch of few layers in a project of many
- **WHEN** the serial measure gives one frame per chunk
- **THEN** the chunks follow each other over every frame, some hold several, and none of several frames
  passes the limit
- **TEST** t1767 (`src/libs/kt-engine/src/export_pipeline.rs`)
