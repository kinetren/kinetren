# Spec Delta

## ADDED Requirements

### Requirement: Live sidechain — stage order by key, key window, no cycles allowed
The compressor in an audio track's chain SHALL accept a KEY — another audio track of the same reel.
The key signal SHALL be that track AS IT SOUNDS: the sum of its events with their weights, FX and gains, its own chain, its level and its muting (mute / not solo). A muted key SHALL give silence in the detector, that is, there will be no ducking — and this is the same thing a person hears.
The key SHALL be computed AT THE POSITION OF THE SAME SAMPLES that the device slot processes: nodes with an analysis window that stand BEFORE it output their results through their rings, so the signal at the slot is older than the chain input by exactly their sum. The detector and the compressed signal MUST be aligned in time, otherwise ducking fires earlier or later than the speech (on the chain "limiter + compressor with key" — 5 ms early). The key branch SHALL have ITS OWN state (its own stream through the key chain), separate from the output stage of the same track: one and the same processor cannot be run twice per frame without losing continuity (invariant 3).
The order of stages SHALL be determined by the key dependency: the key track is computed before the target track; with equal dependencies the previous order holds (row rank), so that the order of f32 additions does not change. A CYCLE (A listens to B, B listens to A, of any length) MUST NOT exist: the model rejects a write that creates a cycle with a named error; the render additionally cuts the recursion — inside the key branch a compressor with a key works as an ordinary one (the detector on its own input). A key pointing at a vanished or non-audio track SHALL be considered absent (the device works as an ordinary compressor), not as a render error.
A track without keys SHALL render in the previous order and with the previous code — byte for byte.
The sidechain MUST NOT add lookahead: ducking has no output delay, and `chain_lookahead` does not depend on the key.
Why: the owner's decision R.63/R.65 — a live device instead of a snapshot; alignment to the target window is the only way not to drift from PDC (R.48); the ban on cycles — because otherwise the frame would be undefined, and "resolving" a cycle silently would mean passing silence off as music.

#### Scenario: Music dips under speech live
- **WHEN** the speech track has two lines, the music track has a steady tone with a compressor whose key points at the speech track; a window covering both lines is rendered
- **THEN** the music is quieter exactly within the speech intervals (taking attack and release into account) and returns to its previous level between them; a project without keys in the same run is byte for byte equal to the previous one
- **TEST** t769 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: Stage order, alignment, and determinism
- **WHEN** the key track stands BELOW the target by rank; the target's chain has a node with lookahead; the same render is run twice, and the second time in pieces with different chunk boundaries
- **THEN** the key is computed before the target regardless of rank, the ducking coincides in time with the speech (there is no shift by the lookahead), both runs are byte for byte equal
- **TEST** t770 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: A cycle, a dead key, muting the key
- **WHEN** an attempt is made to assign keys in a circle; then the key track is deleted; then the key is muted
- **THEN** the cycle command is rejected with a named error and the model is untouched; with a dead key the device sounds like an ordinary compressor (no panic and no silence); with a muted key there is no ducking
- **TEST** t771 (`src/libs/kt-engine/tests/sidechain.rs`), t772 (`src/libs/kt-model/tests/model.rs`)

## MODIFIED Requirements

### Requirement: Track stage — sum, chain, level, mute/solo; without need, the path stays the same
An audio track with a non-empty chain, a level ≠ 0 dB, or muting (mute / not in the solo set) SHALL have a stage: the sum of its events with the same weights (fade × overlap × event gain) → the track's chain (a streaming processor without clip shape: neither source, nor length, nor warp) → × level → × 0 when muted → into the output. A track without a stage SHALL be summed into the output in the previous summation order — byte for byte. Muting is a multiplier, not a removal from the graph: the FX state of the events of a muted track stays alive. Solo SHALL be a set of tracks within a kind; releasing solo returns the previous mutes. The FX-state key SHALL be composite: an event or a track; inserting, deleting and reordering tracks MUST NOT touch the state of the neighbours.
A track's chain SHALL receive lookahead like an event's chain (the summation window extends forward, the output is aligned with the input) and sounds as a tail after the last event (the input is silence). Mute/solo are project content and apply to the export.
A stage SHALL be allowed to depend on ANOTHER stage — through a sidechain key: then the order of computation is set by the dependency, not only by the row rank, and the key branch has its own state.
Apart from the sidechain key, stages have no dependencies: a track does not read a neighbour's output in any other way.
Why: reordering the f32 additions would shift GOLDEN_AUDIO; removal from the graph would destroy the state (invariant 3, defects b/f of the old engine); a key of the form "(kind, number)" would die on track insertion; the owner's decision R.62 — mute and solo in the project; R.65 — a live sidechain whose detector source is a neighbouring stage.

#### Scenario: Without a stage — the bytes are as before; level, mute, and solo are exact
- **WHEN** a project without track properties is rendered; then a level of −6 dB on a track; then mute on a track with reverb on an event, and unmuting in the middle of the tail; then solo on two of three tracks
- **THEN** t2/t3/`GOLDEN_AUDIO` are byte for byte; −6 dB is applied exactly after the weights and the event gain; under mute the track is silent, and after unmuting the reverb tail continues without a cold start; in solo exactly two sound, and releasing solo returns the previous mutes
- **TEST** t732 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: A track's chain on the sum, look-ahead, and tail
- **WHEN** reverb sits on a track of ten lines; a limiter with lookahead sits on the track; the last event has ended
- **THEN** the tail extends across the seams (as t449), the output is aligned with the input (a shift of 0 relative to a track without a chain), the tail rings out after the last event; the cost of the window is measured and does not grow with the project size
- **TEST** t733 (`src/libs/kt-engine/tests/track_fx.rs`)

#### Scenario: The track's state key survives reorderings
- **WHEN** in the middle of a track chain's tail a track is inserted above, then tracks are reordered, then a track is deleted
- **THEN** the state of the chain below is untouched (the output is continuous); after deletion the state is discarded
- **TEST** t734 (`src/libs/kt-engine/tests/track_fx.rs`)

#### Scenario: A stage depends on a stage
- **WHEN** the target track has a compressor with a key on the neighbouring track, and both have their own chains
- **THEN** the key stage is computed first, its own output in the mix is not doubled, and the state of the key branch does not interfere with the state of its output stage (the tails of both are continuous)
- **TEST** t770 (`src/libs/kt-engine/tests/sidechain.rs`)

## REMOVED Requirements

### Requirement: Ducking — a snapshot of the envelope, not a live sidechain
**Reason**: A direct decision by the owner (R.63, confirmed by the answers in R.65): ducking is a live sidechain compressor on a track, shown by the same compressor widget; remove the snapshot entirely. The requirement held exactly the opposite ("the sound of an event MUST NOT become a function of another track in real time") and was explicitly marked "until change `sidechain-ducking-compressor`".

**Migration**: Baked level curves remain ordinary automation and continue to sound; the live device creates them anew. The requirements about the name by industry and about the place of the call (the track window, not its own window) do not disappear — they live in `ui-reach` and `ui-timeline`, while the parameter memory in the user storage is no longer needed: the device parameters now live in the project, in the track's chain, and go through undo.
