## ADDED Requirements

### Requirement: The live decoder serves a moment once, at every size asked
A video span of a `MediaWorker` SHALL carry every decode size asked at its moments and go through
`read_video_span_qualities`, so a picture is decoded once and converted to each size, and a file
read at two places gets its second decoder there. The worker SHALL look into its queue between
chunks of whole moments, never between the sizes of one moment. A span SHALL hold at most half the
video budget, every key counted at its own decode size, and the keys that do not fit SHALL stay
queued. A native-size key SHALL keep a span of its own. The source stays owned by that worker's
thread (invariant 5).
Why: `kt-cli preview` on the showcase (preview-throughput, 2026-11-04): a sharp layer over its
blurred backdrop asks one file at two sizes, and two spans, one per size, sent the second behind
the cursor, which rebuilt the decoder: 3,844–5,413 pictures decoded to deliver about 550, 55–75
reopens in 12 s, 9.5–12.8 of 25 V-fps at 1080p, 42,762–82,482 frames of audio underrun. Keys past
the budget were also taken off the queue and dropped silently.

#### Scenario: Two sizes of one moment are one decode
- **WHEN** a worker is asked 8 moments of one file at two decode sizes
- **THEN** both sizes of every moment reach the cache, no read goes behind the cursor, and the
  decoder produces each picture once
- **TEST** t1520 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A chunk is whole moments
- **WHEN** a span of 10 moments at two sizes is decoded
- **THEN** every read holds at most four moments, and no moment is asked in two reads
- **TEST** t1521 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A span fits the cache at each key's own size
- **WHEN** a tiny key is queued before twelve 4K keys against a cache of three 4K frames
- **THEN** no read holds more than half the budget, and every key is still decoded in the end
- **TEST** t1522 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A sharp layer over its backdrop plays in time
- **WHEN** a file costing 12 ms a picture stands twice at one moment, at two decode sizes, at 25 fps,
  and 4 s are played through the real player against a wall-clock output device
- **THEN** at least 70 % of the grid's frames reach the screen, counted as the performance panel's
  V-fps counts them
- **TEST** t1524 (`src/libs/kt-engine/tests/preview_throughput.rs`)
