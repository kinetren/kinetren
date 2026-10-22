# Spec Delta

## MODIFIED Requirements

### Requirement: Track number within a view = row rank
The tracks of the root and of each reel SHALL be objects with a stable id; the row order is the order of
their vector. Within each kind the rows SHALL go in ascending order of event number; the order of
rows encodes only the alternation of kinds. The single reorder door SHALL renumber everything addressed by the track number (clips, morphers, cameras, foreign raw material, regions), at the root and in each reel; the name, height, switches, level, chain and mode travel
together with the object without renumbering; validation rejects a non-canonical form, reading canonicalizes it.
The sub-mix and the sub-plan of a reel SHALL apply the track stages of the reel; the law E holds
for tracks without stages. There is exactly one exception — the expansion bench (the band at the tail); commands that apply the invariant while the band is open SHALL be rejected and SHALL not collapse the band themselves.
Why: the engine summed layers by number, the screen drew by row order — a track under the ceiling was the bottom layer; three parallel property vectors and six functions for their synchronization — the class "the name exists, the flag does not" after an insertion.

#### Scenario: Reorder renumbers, composition == screen
- **WHEN** the rows are reordered
- **THEN** the contents are renumbered, the order of the composition matches the order of the rows, the region / foreign track / name / height move with them, undo restores
- **TEST** t588 (`src/libs/kt-model/tests/lane_order.rs`), t241 (`src/libs/kt-model/tests/lane_order.rs`)

#### Scenario: Guard and canonicalization on read
- **WHEN** a document or a reel is non-canonical
- **THEN** validation rejects; reading canonicalizes the root and the reels; a file in the owner's project form is read canonically, the foreign raw material of a track moves with the track
- **TEST** t589 (`src/libs/kt-model/tests/lane_order.rs`), t590 (`src/libs/kt-model/tests/lane_order.rs`), t593 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Bench is open
- **WHEN** a reorder of rows or a count of tracks is invoked while a band is open
- **THEN** refusal `BandsOpen`; the band is not collapsed by the refusal
- **TEST** t591 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Track at the top — top layer
- **WHEN** a video track is added from the menu of a morph row and an event is placed on it
- **THEN** the event is drawn as the top layer
- **TEST** t594 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Track id is stable, properties travel with the object
- **WHEN** a track is inserted above, reordered, a neighbour is removed; the name and height are changed
- **THEN** the id is the same, the name/height/switches are in place, changes of name and height are rolled back by undo, t426 (destructuring of the root and of a reel) compiles with the new field
- **TEST** t729 (`src/libs/kt-model/tests/track.rs`)

#### Scenario: Track stages inside a reel
- **WHEN** a track inside a reel has a chain and a level; the same reel without track properties
- **THEN** the instance sounds and looks with the track stage; without properties — the law E byte for byte (t434/t435/t447)
- **TEST** t739 (`src/libs/kt-engine/tests/nest_render.rs`)
