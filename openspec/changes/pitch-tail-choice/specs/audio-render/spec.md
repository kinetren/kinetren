## ADDED Requirements

### Requirement: A split under "rings on" is two streams for the effects before the pitch
With the pitch's echo choice at "rings on" and something ringing before the pitch, a split SHALL NOT
hand the processor on (the predecessor still rings past its end): the left piece's echo rings into the
right piece, and the right piece's effects before the pitch start with it, as a split sounds without
the pitch; the pitch reads across the join on both sides. The render and the prefetch SHALL read and
order the same frames in both choices (the window functions do not depend on the choice). With
nothing ringing before the pitch the hand-off SHALL apply as before.
Why: one stream across a split is possible only for a chain whose state at the join is the next
piece's state; a ringing predecessor is still sounding while the next piece starts.

#### Scenario: A split with an echo before the pitch, rings on
- **WHEN** a 1.5 s event with an echo before a pitch +7 ("rings on") is cut into fifteen 100 ms pieces, rendered from the start in one piece and in pieces of 1 024 and 333 frames, with a recording fetcher; a loop region over pieces 3–9 plays four laps
- **THEN** no join hands the processor on; no 64-frame window within 1 024 frames of a join falls below half the steady level; one piece == pieces; every source frame read is inside the prefetch request; the laps after the first are byte for byte equal
- **TEST** t1335 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: The ringing echo survives a seam and a seek
- **WHEN** a "rings on" event with an echo before the pitch is played through its out-point, a loop region covering its end and 200 ms past it wraps four times, and a seek lands in its echo
- **THEN** the laps after the first are byte for byte equal and the output past the out-point on every lap carries no frame of the material after the out-point (equal over a source loud and silent there); the seek equals a cold render of that position
- **TEST** t1339 (`src/libs/kt-engine/tests/pitch_tail.rs`)
