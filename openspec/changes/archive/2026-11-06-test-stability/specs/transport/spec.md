## MODIFIED Requirements

### Requirement: A start begins with its sound
A start (opening, a seek, a change of direction, a loop flip) SHALL hold the render until the audio
of a whole ring and the look-ahead margin is in the cache, at most `START_AUDIO_GRACE`, and SHALL then
go on with what there is. The clock SHALL stand until the first frame reaches the output. A pass of
the callback before that first frame, with the ring empty, SHALL NOT count as an underrun.
Why: the render filled the ring in one pass when Play landed, ahead of the decoders: 0.34 s of silence
on the first play, on 1 start of 4 of the showcase (`docs/found-defects.md` row 11; cold-start-audio,
2026-11-04).

#### Scenario: A cold start
- **WHEN** a range plays with no warm-up from a source that takes 4 ms a burst
- **THEN** no frame of it is an underrun, and it plays in real time
- **TEST** t1537 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A source that does not answer
- **WHEN** a source does not answer until the play has ended
- **THEN** the start goes on after the grace, and the play ends with its underruns counted
- **TEST** t1538 (`src/libs/kt-engine/tests/preview_throughput.rs`)
