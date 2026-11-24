## ADDED Requirements

### Requirement: A track stage pre-rolls on a real seek
On a real seek (a generation change, a sign flip of the rate, a loop flip) every track stage whose lane
sounds within its tail and look-ahead before the new head SHALL reset and run its lane's sum over
`[head − tail − look-ahead, head)` through its chain into the void, its events positioned at the pre-roll's
start by their own seek, its keyed compressors' key branches pre-rolled over the same window through their own
copy of the key track's chain (the window covering the key chain's tail, look-ahead and slot delay too),
on the render thread; a stage whose lane is silent there SHALL do nothing. A seam
(a loop wrap, an event moved during play) SHALL stay a seam. An export of a range SHALL equal the whole
export's samples of that range byte for byte. The start SHALL count as warm for the kept mix: there is no cut
pre-roll (the owner's choice), and the start's prefetch SHALL cover the pre-roll window.
Why: found-defects 420 — a seek or an export range starting inside a track's reverb or delay tail played
silence where continuous play rings; event chains already pre-roll (track-stage-preroll D1–D4). The owner,
kkt-26.11.20 A15: "Всегда полный хвост".

#### Scenario: A seek into a track reverb rings as continuous play
- **WHEN** a track with a 2 s delay without feedback carries a short event, and play seeks to 0.5 s after the event's end
- **THEN** the samples after the seek equal the continuous render's samples at the same frames
- **TEST** t3361 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: A silent lane costs no pre-roll
- **WHEN** play seeks into a track whose lane is silent for longer than its tail before the head
- **THEN** no lane read is made before the head and no pre-roll is counted
- **TEST** t3362 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: An export range equals its part of the whole export
- **WHEN** a project is exported whole and again from a point inside a track delay's repeats (no feedback), and a reverb track's tail is checked within its tolerance
- **THEN** the range export's audio equals the whole export's samples of that range byte for byte, and GOLDEN_AUDIO is unchanged
- **TEST** t3363 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: A keyed compressor's key pre-rolls
- **WHEN** play seeks to a point where a keyed compressor on a music track is ducked by speech that started 0.3 s before
- **THEN** the ducking at the seek point equals continuous play's
- **TEST** t3364 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: The kept mix keeps the start after a full pre-roll
- **WHEN** a seek into a track reverb's tail is played, then played again
- **THEN** the first play keeps its start (`keep_from` is the head; t2373's cold start is gone), and the second hands it out from the kept mix
- **TEST** t3365 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: A sign flip pre-rolls, a wrap stays a seam
- **WHEN** the rate flips sign inside a stage's tail, and a loop over the same track plays three laps
- **THEN** the flip pre-rolls the stage, and the laps count no pre-roll after the first
- **TEST** t3366 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: The start's prefetch covers the pre-roll window
- **WHEN** play starts on a cold cache at a point inside a track delay's repeats
- **THEN** the start's hold waits for the window before the head as well, the pre-roll counts no warm-up miss, and the first chunk is kept
- **TEST** t4103 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: The trace says what a stage pre-roll cost
- **WHEN** a run seeks twice into a track reverb's tail and once into silence, and its trace is read
- **THEN** two stage pre-roll records carry their frames and microseconds, none for the silent seek, and `kt-cli trace summary` sums them
- **TEST** t4104 (`src/apps/kt-cli/src/trace_query/tests.rs`)
