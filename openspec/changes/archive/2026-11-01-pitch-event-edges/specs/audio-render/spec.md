## ADDED Requirements

### Requirement: A split of a pitched event is one stream
An event SHALL know its run predecessor: the event on the same track that ends exactly where it
starts, plays the same media from the frame right after (`prev.source_in + prev.len == source_in`),
has the same chain structure and look-ahead, both chains read across edges (audio-fx), neither is
reversed or overlapped, no transition window lies at the join and the predecessor has no tail. When
such an event is entered fresh at its first frame while the predecessor's processor has just played to
its own end, the render thread SHALL hand the predecessor's processor on to it (a move in the store,
rebased by the predecessor's length) instead of starting it cold, and give the predecessor a reset
processor. A tail-less chain that reads across edges, entered again at its first frame after it
stopped, SHALL start fresh (the hand-off or a seek with the context pre-roll), not continue through a
seam. The decision depends on the model and on the play order only, never on slicing or timing.
Why: two pitch streams have independent grain grids, WSOLA offsets and vocoder phases, so a cut
between them steps even with full rings; a split of one material is one stream, and so it sounds
as the uncut event. A seam keeps a ring only when the output was continuous; an event that was silent
since it stopped holds a ring of where it stopped.

#### Scenario: Fifteen pieces of one event sound as the event
- **WHEN** a 1.5 s pitched event (+7 and −7 semitones, each engine) is cut into fifteen 100 ms events in place, and both are rendered from the timeline start in one piece, in bursts of 1 024 and in pieces of 333 frames
- **THEN** the fifteen events give the uncut event's output byte for byte, in every slicing, twice in a row; the same render before this change had 320 frames of silence at every join (WSOLA +7)
- **TEST** t1105 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: No hand-off where the run breaks
- **WHEN** the run of t1105 has an 8-frame gap on the timeline at join 3, a source that skips 8 frames on the 7th event (joins 6 and 7), a different chain structure on the 10th event (joins 9 and 10) and an echo with a tail on the 13th (joins 12 and 13)
- **THEN** exactly those joins start the next event fresh (its own context pre-roll), every other join hands the processor on; no join dips below half the steady level; the render is the same in one piece and in pieces
- **TEST** t1111 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: A seek into the run equals a cold start there
- **WHEN** the fifteen-event run is sought to the middle of its eighth event and to the exact start of its eighth event
- **THEN** each equals a cold render of that position (a fresh processor and its context pre-roll); continuous play through the same join equals the uncut event (the hand-off), which a seek does not have to
- **TEST** t1110 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: A loop over the run
- **WHEN** a loop region covers events 3–9 of the run exactly (its ends on their edges) and plays four laps, and another region starts in the middle of event 3 and ends in the middle of event 9
- **THEN** there is no 64-frame window below half the steady level anywhere after the first frame (the wrap is a cut between two events, without silence on either side); the laps after the first are byte-for-byte equal to each other; one piece == pieces
- **TEST** t1112 (`src/libs/kt-engine/tests/pitch_edges.rs`)

## MODIFIED Requirements

### Requirement: Chain lookahead (PDC) — input runs ahead of output
An effect with an analysis window SHALL read the source FORWARD of the output position and deliver the output aligned — for the rest of the engine there is no delay, the playhead and crossfades do not move. The look-ahead is included in the seek warm-up and in the source window, but MUST NOT enter the tail (`chain_tail`) — otherwise the event would sound after its end. The look-ahead formula of a kind lives in one home and matches between the node and the static function; La < 4800 frames.
When La = 0 the arithmetic SHALL be identical to the path without look-ahead (`GOLDEN_AUDIO`).
The loop region's wrap SHALL fold the look-ahead window as it folds the output: the chain input of
an event that covers the region end continues from the region start `La` frames before the output
wraps, and the source window and the prefetch request fold the same way (one function for both).
An event whose chain reads across edges (audio-fx) and which starts INSIDE the region is not what
sounds after the wrap: it SHALL NOT fold — its input continues into its own material, and the event at
the region start enters fresh (pitch-event-edges D5).
A track's chain with look-ahead SHALL obey the same rule on the summed input of the track.
Why: an output delay would shift the sound away from the picture, and a tail that included the analysis window would extend the event. A folded input that lands before the event's material is silence, and a pitch node fed silence past the region end dips exactly as at an edge.

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
