## ADDED Requirements

### Requirement: A decode request lives until it is served, and the decoder reads a run forward
The MediaWorker SHALL NOT queue again, nor count as a request (`decode_requests`), a video key that is
already queued at that priority or already taken into the span being decoded; only a key that enters the
queue is a request. When nothing is queued forward of the decoder's cursor, it SHALL go back to the START
of the run of queued keys ending at the nearest one behind the cursor (the same priority, at most one span's
moments, at most 2 s back, the same sizedness) and read that run forward, never one key per span from the
run's end.
Why: the owner's journal, kkt-26.11.08 (release 360b50e6): after he selected Datamosh, 1570 decode requests
a second while paused, 15–100 frames decoded, every one dropped unread. Each composite re-asked ~392 keys
that were already queued, and with the cursor past them the decoder walked the history backward, a seek and
a decode from the keyframe per frame (decode-request-storm design, the mechanism).

#### Scenario: A run behind the cursor is read forward
- **WHEN** the decoder has read frame 400 and then 40 keys 100…139 are queued behind it, on a model decoder that counts its seeks and pictures
- **THEN** every key is served once, the first served of them is 100, at most 2 reopens and at most 40 + one GOP pictures are decoded
- **TEST** t2040 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A key queued or in flight is not a new request
- **WHEN** four keys go into the decoder's span, held at a gate, and keys 0…19 are then submitted fifty times
- **THEN** the request counter grows by 20, and every key is decoded exactly once
- **TEST** t2041 (`src/libs/kt-engine/tests/decode_storm.rs`)
