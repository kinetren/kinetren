## ADDED Requirements

### Requirement: The stream's sound reads the picture's schedule compiled once
`datamosh_sound` SHALL compile the picture's schedule over its event's layout once for its rows, curves
and placement, and read it for every codec frame, with the bytes the schedule built per codec frame
gave; a change of the rows, the curves or the placement SHALL compile it again.

Why: the audio half built a layout and a schedule per 1024-sample frame and walked a gate's run back,
reading the curves at every position, for each (engine audit, row 8).

#### Scenario: The sound keeps its bytes
- **WHEN** 6 s of a tone go through the stream at 25 fps under mosh, reset, bloom, freeze, loss, residual and quantiser curves, with a seek back into the middle, twice
- **THEN** both runs are byte-equal and equal the hash pinned from the per-frame schedule
- **TEST** t2272 (`src/plugins/fx-video-codec/tests/schedule.rs`)
