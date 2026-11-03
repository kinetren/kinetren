## ADDED Requirements

### Requirement: The echo of the effects before the pitch — cut at the event's end or rings on
An edge-reading kind (pitch) SHALL carry a choice "echo of the effects before the pitch": **cut at
the event's end** (the default, written nowhere) or **rings on after the event**. "Cut" SHALL be the
sound of the context as it is (the nodes before the pitch process the material on both sides, the
content gate cuts the result to the event) byte for byte. "Rings on" SHALL leave everything up to the
out-point byte for byte as "cut", and from the out-point on the output SHALL be the chain up to the
pitch run on the event's own sound followed by silence — split off from the running state exactly
where the input reaches the out-point — so the echo past the end contains no frame of the material
after the out-point and starts at the out-point without a hole. When nothing before the pitch rings,
"rings on" SHALL be "cut" byte for byte. A chain whose state cannot be split off SHALL stay cut at the
end.
Why: the owner's events carry their own Slowed+Reverb under a morpher's pitch; since the pitch reads
across the edges, the reverb's echo was cut with the event, and asked about it the owner wanted both
("make an option so it can be done this way and that way", 2026-11-01). One pitch stream cannot both
read the next material for its last grains and keep it out of the echo (the earlier rejected
alternative: ≈ 40 ms of foreign sound after every event or a 96 ms hole); a second stream split off at
the out-point does both. The start stays the cut's start: restarting the effects before the pitch at
the in-point (no echo of the material before it) put a step into the pitch's input, and the vocoder
fell to 0.29 of its level at every event's start.

#### Scenario: The echo rings on and carries nothing of the next material
- **WHEN** an event (±7, each engine) with an echo before the pitch stands over a source that is loud past its out-point, and over the same source silent past its out-point, with "rings on" and with "cut"
- **THEN** with "cut" the output past the out-point is zero; with "rings on" it is not, and it is byte for byte the same over both sources (no frame of the next material); the body before the out-point equals the "cut" body; no 64-frame window in the first 1 024 frames past the out-point is a hole (below a tenth of the echo's level there); one piece == pieces
- **TEST** t1331 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: The joins of "rings on" keep the "cut" bodies
- **WHEN** fifteen events of other material with an echo before the pitch (±7) and the owner's five copies with Slowed+Reverb and a morpher's pitch +5 are rendered with "rings on", with each engine, and each event alone
- **THEN** every event's body equals its "cut" body and no 64-frame window within 1 024 frames of its own edges falls below half of its steady level; the reverb of each copy rings on past it; nothing changes before the first out-point; one piece == pieces of 1 024 and 333 frames
- **TEST** t1333 (`src/libs/kt-engine/tests/pitch_tail.rs`)

#### Scenario: "Cut" is the former sound; "rings on" without an echo before the pitch changes nothing
- **WHEN** a pitch's echo row is set to "cut" explicitly; pitch alone, a limiter before the pitch, an echo AFTER the pitch and a neutral pitch after an echo are rendered with "rings on" and with "cut"
- **THEN** the explicit "cut" is the default slot; each "rings on" render is byte for byte its "cut" render; with an echo before an active pitch the two differ
- **TEST** t1334 (`src/libs/kt-engine/tests/pitch_tail.rs`)

### Requirement: An audio state can be split off
An audio state SHALL be able to give a copy that continues exactly as it would on the same input
(`fork`); a state that cannot SHALL say so, and the host SHALL then not ring the echo of its chain
past the event's end. Every built-in audio kind SHALL fork.
Why: "rings on" splits the chain up to the pitch at the out-point; a copy is the only exact form that
costs nothing between event ends.

#### Scenario: Every built-in audio kind forks and continues as itself
- **WHEN** every built-in audio kind (defaults, then every row at 70 %, each pitch engine) has processed a second of a tone, is forked, and both copies process the same next second; then one copy gets different input
- **THEN** both outputs of the same input are byte for byte equal, and feeding one copy leaves the other untouched
- **TEST** t1330 (`src/libs/kt-engine/tests/pitch_tail.rs`)
