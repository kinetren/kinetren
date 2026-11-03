## ADDED Requirements

### Requirement: A loop lap of a chain with a tail before the pitch starts as playing from there
An event whose chain reads across its edges (audio-fx) and has a tail, entered again at its first
frame or after its out-point by a loop wrap (not as the expected input-first wrap of an event covering
the region end), SHALL start the lap as a seek to that point does: a reset and the context pre-roll.
What the previous lap still rings past its out-point SHALL go on in the processor that played it,
summed over the new lap at the tail zone's weight, until its tail ends; at most four earlier laps ring
at once (the oldest is dropped), and a seek drops them. Without a loop the output SHALL be unchanged.
Why: a seam kept the pitch's look-ahead ring of where the previous lap stopped — ≈ 75 ms of silence
with "rings on", half the level and sound from past the out-point with "cut" — while a seek reads what
will sound; the previous lap's echo rings on as two copies of the event one after another do.

#### Scenario: Every lap starts as the play from there
- **WHEN** an event with an echo (100 ms) before a pitch (±7, each engine, "cut" and "rings on") is looped by a region starting at its first frame or 50 ms inside it and ending 200 ms past its out-point (the echo has rung out by the wrap), four laps in pieces of 1 024 and 333 frames
- **THEN** every lap is byte for byte the play from the region start; both slicings are equal; before the change the deepest 64-frame window of a lap's first 1 024 frames was 0.000 of it with "rings on" and about 0.45 with "cut"
- **TEST** t1440 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: Earlier laps ring on over a new lap
- **WHEN** the region ends 50 ms past the out-point (the echo, without and with 0.6 feedback, still rings at the wrap), seven laps, each engine, both choices, in pieces of 1 024 and 333
- **THEN** every lap is the play from the region start plus the play continued past the region end of each of the last four laps, summed oldest first, byte for byte; with "cut" the lap is the play from there alone; no lap starts below half of the play from there; both slicings are equal
- **TEST** t1441 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: Live — loop and play over such an event
- **WHEN** the real program opens our fixture (a 440 Hz event at 1.0 s with an echo and a +7 pitch at "rings on"), a region is dragged on the ruler from the event's start to 0.4 s past its end, L turns the loop on and Space plays three laps; then the same region is played on repeat by the engine (`kt-cli probe --time`)
- **THEN** a screenshot per step; the cursor wraps three times to the event's start and stays in the region, the program still plays; the engine's four looped laps have no miss
- **PATH** open the project → drag on the ruler 1.0 s → 2.0 s: the region → L: the loop button lights → Space: the cursor runs to the region end and back to the event's start, three times → Space stops
- **TEST** t1442 (`src/scripts/live/looptail.py`)
