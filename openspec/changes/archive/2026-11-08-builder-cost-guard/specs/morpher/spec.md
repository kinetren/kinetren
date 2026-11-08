## ADDED Requirements

### Requirement: The cut pass lays the forbidden zones out once
The forbidden zones of a reel's events (`ZoneIndex`) SHALL be laid out once per layer pass —
transitions by the events they name, events by track sorted by start with the latest end so far —
and answer each target in the order the scan answered (`snap_out_of` takes the first zone holding
the point). `forbidden_zones(reel, clip)` keeps its answer.
Why: each cut target walked every transition and every event of its reel, per cutting layer.

#### Scenario: A cutting layer over events with transitions
- **WHEN** a layer with a time warp in its patch cuts 24, 96 and 384 butted events with crossfades
- **THEN** `effective_clips` and the builds grow at most 8× and 32×
- **TEST** t2066 (`src/libs/kt-engine/tests/build_cost.rs`)
