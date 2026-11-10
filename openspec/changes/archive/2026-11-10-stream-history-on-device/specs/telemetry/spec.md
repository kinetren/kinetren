## ADDED Requirements

### Requirement: The trace says when a compute submission ran on the device, on the CPU clock
Every compute job SHALL leave a record of when its submission was made and when its first dispatch
started and its last ended on the device, both on the trace's own CPU clock: the device's timestamps
mapped through a calibration (a timestamp written by an otherwise empty submission, bracketed by the
CPU clock before the submission and after its completion; the narrowest bracket of the recent ones
wins, and the record carries its width as the error). A device without timestamps SHALL say so in the
record instead of a start. `kt-cli trace gpu` SHALL report, per class, how long submissions waited
before their device start and how long the device took, so a late start (a queue behind other work)
is told from a late completion signal.
Why: found-defects 398 — his trace could not tell whether a 46 ms round trip for a 0.2 ms job was spent
waiting to start or waiting to be told it had finished.

#### Scenario: A start lies between the submission and the completion
- **WHEN** compute jobs run on a device with timestamps, one after another and behind a long job
- **THEN** each record's device start is at or after its submission minus the calibration error and its end at or before the moment its buffers came back plus the error; the job behind the long one starts late by about the long one's device time; `trace gpu` prints the start delay per class
- **TEST** t2357 (`src/libs/kt-gpu/tests/compute_start.rs`), t2362 (`src/apps/kt-cli/src/trace_query/tests.rs`)

### Requirement: The trace names each batch of a stream history walk
Each submission of a stream history walk SHALL be a `stream_walk` record: its first position, the
positions in it, the pictures it read back and the checkpoints it kept; `kt-cli trace loop` SHALL count,
per lap, the positions computed and the distinct positions among them.
Why: found-defects 394 — "1413 positions computed, 618 distinct" had to be reconstructed by hand from
job records.

#### Scenario: A walk is its batches
- **WHEN** a 40-position mosh is walked cold in batches
- **THEN** the trace holds one `stream_walk` record per submission whose positions sum to 40, and `trace loop` reports 40 computed, 40 distinct
- **TEST** t2363 (`src/plugins/fx-video-codec/tests/walk.rs`), t2364 (`src/apps/kt-cli/src/trace_query/tests.rs`)
