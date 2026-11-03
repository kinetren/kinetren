## ADDED Requirements

### Requirement: A split under "rings on" rings only at its end
With the pitch's echo choice at "rings on", an event whose material the next event on its track
continues frame for frame (the run relation of "a split is one stream", except that the predecessor
may ring) SHALL NOT ring its echo past its out-point: the next piece's context already carries that
echo, as the uncut event would. A split under "rings on" therefore sounds as under "cut" up to its last
piece's end, and the last piece rings on. A predecessor that rings hands nothing on. The render and
the prefetch SHALL read and order the same frames in both choices.
Why: one stream across a split is possible only for a chain whose state at the join is the next
piece's state; a ringing left piece and the right piece's context would sound the echo of the join's
material twice.

#### Scenario: A split rings only at its end
- **WHEN** a 1.5 s event with an echo before a pitch (±7, each engine) is cut into fifteen 100 ms pieces in place, rendered with "rings on" and with "cut"
- **THEN** up to the last piece's end the two are byte for byte equal; past it "cut" is silent and "rings on" rings
- **TEST** t1332 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: A split under "rings on" — hand-off, prefetch, loop
- **WHEN** the fifteen pieces with "rings on" are rendered from the start with a recording fetcher, in pieces of 1 024 and 333 frames, and a loop region over pieces 3–9 plays four laps in pieces of 1 024 and 333
- **THEN** no join hands the processor on; no join dips below half the steady level; one piece == pieces; every source frame read is inside the prefetch request; the loop is the same in both slicings and its last lap sounds
- **TEST** t1335 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: The ringing echo through a seam and a seek
- **WHEN** a "rings on" event with an echo before the pitch is looped by a region holding it and 200 ms past it for four laps, over a source loud and silent past its out-point; a seek lands in its echo
- **THEN** on every lap the echo rings past the out-point and is the same over both sources; the loop is the same in pieces of 1 024 and 333; the seek equals a cold render of that position
- **TEST** t1339 (`src/libs/kt-engine/tests/pitch_tail.rs`)
