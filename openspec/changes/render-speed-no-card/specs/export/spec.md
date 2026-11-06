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
