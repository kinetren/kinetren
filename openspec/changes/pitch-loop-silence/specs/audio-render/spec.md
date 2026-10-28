## MODIFIED Requirements

### Requirement: Chain lookahead (PDC) — input runs ahead of output
An effect with an analysis window SHALL read the source FORWARD of the output position and deliver the output aligned — for the rest of the engine there is no delay, the playhead and crossfades do not move. The look-ahead is included in the seek warm-up and in the source window, but MUST NOT enter the tail (`chain_tail`) — otherwise the event would sound after its end. The look-ahead formula of a kind lives in one home and matches between the node and the static function; La < 4800 frames.
When La = 0 the arithmetic SHALL be identical to the path without look-ahead (`GOLDEN_AUDIO`).
The loop region's wrap SHALL fold the look-ahead window as it folds the output: the chain input of
an event that covers the region end continues from the region start `La` frames before the output
wraps, and the source window and the prefetch request fold the same way (one function for both).
A track's chain with look-ahead SHALL obey the same rule on the summed input of the track.
Why: an output delay would shift the sound away from the picture, and a tail that included the analysis window would extend the event.

#### Scenario: Look-ahead does not shift the output
- **WHEN** a limiter in linear mode (threshold and ceiling at 0 dB, window 5 ms) stands in the chain
- **THEN** the output equals the chain without it, sample for sample
- **TEST** t334 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: The window works, the event does not sound longer
- **WHEN** a limiter with a threshold of −6 dB
- **THEN** the peak is exactly at the ceiling, the reduction completes before the peak, and `ext_end` has not grown
- **TEST** t335 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: Neutral is byte-for-byte and independent of slicing
- **WHEN** a pitch shifter at zero parameters without curves; a pitch shifter that is active, with the render requested in one piece and in pieces
- **THEN** the neutral does not touch the buffer and requires no look-ahead; one piece == pieces for both engines (the grids are absolute from the event position); the pitch shift is audible on both engines
- **TEST** t337, t338, t339 (`src/libs/kt-engine/tests/audio_pitch.rs`)

#### Scenario: A look-ahead limiter at a loop wrap plays the region head, not what lies past the end
- **WHEN** a limiter with a 5 ms window and a −6 dB threshold stands on an event whose source is a 220 Hz tone up to the region end and a 1 kHz tone after it; the loop region plays three laps
- **THEN** after each wrap the output carries no 1 kHz component (below −60 dB relative to the tone) and equals, from the first frame after the wrap, the limited head of the region within the limiter's smoothing; the same render today plays 240 frames of the 1 kHz tone after each wrap (the red proof)
- **TEST** t909 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: A track chain with look-ahead at a loop wrap
- **WHEN** a pitch shift and a look-ahead limiter stand on a TRACK of three events, the loop region covers the joint of two events and plays three laps
- **THEN** there is no silent run of 64 frames or more and no material from past the region end after any wrap; the output is aligned with the same track without a chain (shift 0)
- **TEST** t911 (`src/libs/kt-engine/tests/track_fx.rs`)
