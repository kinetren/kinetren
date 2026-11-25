## ADDED Requirements

### Requirement: A picture stands on no disk call
No frame of the preview SHALL wait for the disk without a bound, nor for a lock held across a disk call. The
pass-time table SHALL be read and written by a thread of its own with the table free; the displayed frame
SHALL wait for a moment of the disk tier at most a bound (a quarter of a second while playing) and then
compose it, asking the disk nothing more until that read has ended, whose picture is kept when it comes.
Why: found-defects 835 — the render-ahead producer wrote the pass times inside the device's gate, and the
displayed frame stood behind it for 4.8 s.

#### Scenario: The pass times' file stands
- **WHEN** the table's file is a pipe nobody reads, then one nobody writes, and frames note and ask times meanwhile
- **THEN** every note and prediction returns; the write carries the lines the file had and the measured ones; a file that comes late comes in under what was measured
- **TEST** t3250 (`src/libs/kt-gpu/src/seeds.rs`)

#### Scenario: The disk tier stands
- **PATH** play a part whose moments are on a disk that answers nothing → the picture goes on
- **WHEN** the segment of a moment opens only when the test lets it and moments are asked of the preview
- **THEN** each is composed complete and only the first read was asked; after the disk answers the late picture is kept and the next moment comes from disk
- **TEST** t3252 (`src/libs/kt-engine/tests/render_cache.rs`)
