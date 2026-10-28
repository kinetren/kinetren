## MODIFIED Requirements

### Requirement: Lookahead — input runs ahead of output, no delay for the host
An effect with an analysis window SHALL read the source ahead of the output position and deliver its output aligned: for the rest of the engine the delay is zero, the playhead and crossfades do not move. The source window SHALL be extended forward by the look-ahead in ONE function, which both the render and the prefetch ask. Changing the look-ahead window (and the pitch engine) is structural: the processor is rebuilt. The total look-ahead MUST be less than the minimum seek pre-roll (4800 frames).
Why: the output delay would drift against the master clock and neighboring tracks; a window extended only for the render would produce a silent cache miss — beyond the window the source would return silence without an underrun.

At a wrap of the loop region the look-ahead SHALL read the NEXT LAP: the chain's input crosses the
region end `La` frames before the output does and continues from the region start, so the frames a
look-ahead node holds at the moment the output wraps are the head of the region — the material that
will actually sound. A look-ahead node's rings and grids SHALL survive every seam (a loop wrap, a
clip shifted during playback, a re-anchored material): they are keyed by the continuous position of
the stream since the last real seek, while the node's curves are read at the clip position of the
grain. A loop wrap MUST NOT produce a silent run or material from beyond the region end, for any
region length, including a region shorter than the look-ahead window.
Why: the pitch node re-initialised its rings on every position jump and the source was re-seated only
when the output wrapped, so the first `La` = 4 608 frames (96 ms) after each wrap were synthesised from
an empty ring — "on a loop a piece of silence is always heard, and on a small loop total silence"
(ktprompts v26.10.18 item 7); the limiter's ring meanwhile emitted material from past the region end.

#### Scenario: Look-ahead does not shift the output
- **WHEN** a limiter with a 5 ms window sits in linear mode (threshold and ceiling at 0 dB)
- **THEN** the output equals the chain without the limiter, sample for sample
- **TEST** t334 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: The ceiling is guaranteed and the reduction starts before the peak
- **WHEN** a limiter with a −6 dB ceiling processes clicks
- **THEN** the peak is exactly at the ceiling, and the gain drops within the window before the peak
- **TEST** t335 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: Prefetch covers the look-ahead window
- **WHEN** a chain with a look-ahead node is rendered
- **THEN** everything read from the source enters the prefetch request (`wanted ⊇ read`)
- **TEST** t336 (`src/libs/kt-engine/tests/audio_lookahead.rs`)

#### Scenario: A loop with pitch has no silent run
- **WHEN** a clip of a 440 Hz sine carries a pitch shift of +7 semitones (WSOLA, then the vocoder), a loop region of 1 s that is aligned neither to a burst nor to the pitch hop plays four laps through the looped render in bursts of 1 024 frames
- **THEN** after the first output frame there is no run of 64 or more frames below −80 dBFS anywhere, in particular none after a wrap; the RMS of every lap is within 1 dB of the first lap; the same render today shows a run of about 4 600 frames after each wrap (the red proof)
- **TEST** t906 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A short loop with pitch still sounds
- **WHEN** the same pitched clip plays loop regions of 700 frames (shorter than a burst), 3 000 frames (shorter than the look-ahead) and 6 000 frames (just longer than it), twenty bursts each
- **THEN** every burst carries signal (RMS at least half the dry RMS of the same tone), no burst is all zeros; there are no repeated warm-ups and no underruns (as t167)
- **TEST** t907 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: The looped pitch is deterministic, sliced or not, and reads only what was ordered
- **WHEN** the four-lap looped render of t906 is run twice, once in bursts of 1 024 and once in pieces of 333 frames; and the source reads during it are compared with the prefetch request for the same looped window
- **THEN** both runs and both slicings are byte-for-byte equal; the first lap up to `region end − La` equals the plain (non-looped) render of the same range; everything read from the source is in the prefetch request (`wanted ⊇ read`)
- **TEST** t908 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: A real seek and a moved clip with pitch
- **WHEN** a pitched clip is sought cold to a position and rendered; the same position is reached by continuous playback; then the clip is shifted on the timeline during playback (a seam that is not a loop wrap); and a neutral pitch (zero semitones, no curve) plays a loop
- **THEN** the seek equals the cold render of that position; the shifted clip continues without a silent run of 64 frames or more; the neutral pitch leaves the looped buffer byte-for-byte equal to the dry loop and asks for no look-ahead
- **TEST** t910 (`src/libs/kt-engine/tests/audio_pitch.rs`)
