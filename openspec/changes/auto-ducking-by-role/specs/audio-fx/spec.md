## ADDED Requirements

### Requirement: The compressor's key may be a set of roles
An audio track and an audio event SHALL carry an optional role (Dialogue, Music, Effects, Ambience, Title sound);
an event's effective role SHALL be its own, else its track's. The compressor's key SHALL be either one audio track
(R.65) or a set of roles; a role key SHALL be the sum, per render window, of the post-chain outputs of every event
of the same reel whose effective role is in the set and which is not on the keyed track, computed from outputs the
render already has (no second decode, no second chain run). Moving a key event to another track SHALL NOT change
the output; a key that would make a cycle SHALL be refused with a named error.
Why: the owner, 2026-10-30, question 10 — sections "по типу клипа"; a track key breaks when a line moves track
(F-AUD-007 trick 1).

#### Scenario: A role key equals a track key on the same material, and follows a moved line
- **WHEN** a music track is ducked by a compressor keyed by the speech track, then by the role Dialogue with the speech events tagged; then one speech event is moved to a second audio track
- **THEN** the two renders are equal byte for byte; after the move the role-keyed render is unchanged, while the track-keyed one no longer ducks under the moved line
- **TEST** t3911 (`src/libs/kt-engine/tests/sidechain_roles.rs`)

#### Scenario: A role cycle is refused by name
- **WHEN** a track whose own events are Music is given a key Music, and two tracks key on each other's roles
- **THEN** both commands are refused with `SidechainCycle` naming the track, and the project is unchanged
- **TEST** t3912 (`src/libs/kt-model/tests/roles.rs`)

### Requirement: The ducking form speaks dB of reduction and drives the same compressor
The Auto-ducking form SHALL set the compressor so that a key at the sensitivity level reduces the background by
exactly the "Reduce by" amount, attack and release from one Fade, and "Between them: raise by" as the track
level raised and the reduction deepened by the same amount; it SHALL write only the compressor's own rows (no
second effect kind); an edit of a compressor row under "Details" SHALL mark the form custom, not be overwritten.
Why: the owner — "очень не очевидно что и куда крутить"; rule 24.

#### Scenario: Reduce by 12 dB means 12 dB
- **WHEN** the form is set to Reduce by −12 dB, Sensitivity 50 %, Fade 200 ms and a steady key at the sensitivity level plays under a steady background; then "raise by 6 dB" is added
- **THEN** the steady-state reduction is 12 dB (±0.2); with the raise the level outside the key is +6 dB and under it unchanged (±0.2)
- **TEST** t3913 (`src/libs/kt-engine/tests/sidechain_roles.rs`)

### Requirement: The drawn reduction is the device's own
The reduction drawn on background events SHALL be computed off the UI thread by the compressor's detector and gain
code, cached by the content hash of the key events, the background's chain and the reel, and SHALL match the
rendered gain within 0.5 dB; "Convert to volume curve" SHALL write it as gain keys and switch the compressor off in
one undo step.
Why: Q1 (a); the cache rule — keyed by what determines it.

#### Scenario: The band matches the render; converting keeps the sound
- **WHEN** a ducked background is analysed, rendered, then converted to a volume curve and rendered again
- **THEN** the band and the rendered gain differ by at most 0.5 dB over the project; the converted render differs from the live one by at most 1 dB; one Undo restores the compressor
- **TEST** t3914 (`src/libs/kt-engine/tests/sidechain_roles.rs`)
