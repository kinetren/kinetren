## ADDED Requirements

### Requirement: A multicam composition groups synchronized angles
"Create multicam" SHALL build, in one undo step, a composition whose angles are video tracks (each with
its linked audio) aligned by sound, by timecode or by start, and replace the selected clips by one
instance spanning them; an alignment by sound that cannot find an angle's offset SHALL be refused naming
the angle. Per-angle effects SHALL live on the angle's track inside the composition.
Why: the owner's M1 "не нужно было страдать при сведении камер"; sync exists (F-SPD-004).

#### Scenario: Three cameras and a recorder become one instance
- **WHEN** three camera clips with sound offset by 1.2 s, 0.4 s and 0 s and a recorder clip are selected and grouped by sound
- **THEN** a composition with three angles and the recorder track is created, the offsets match the known ones within one audio granule, one instance replaces the four clips in one undo step; a camera without sound in the selection refuses the grouping by sound naming it
- **TEST** t3505 (`src/libs/kt-model/tests/multicam.rs`)

### Requirement: An instance plays one angle and its chosen sound
An instance of a multicam composition SHALL compose only its active angle's video track and play the
sound its audio choice names (a fixed angle's sound, a non-angle track, or the active angle's sound when
following); the other angles SHALL cause no decode, prefetch or coverage. Cut-and-switch SHALL split the
instance at the displayed frame and set the right piece's angle; switch-only SHALL set the angle of the
piece under the playhead; switching SHALL work while playing, and one pass from Play to Stop SHALL be one
undo step. With a fixed sound, a cut SHALL leave the sound byte-identical to the uncut instance.
Why: invariant 4 (no work for what is not shown); F-TL-005 trick 2 (the sound must not jump); UI rule 2.

#### Scenario: Only the active angle is decoded
- **WHEN** a 9-angle multicam instance plays with angle 3 active and the angle viewer closed
- **THEN** decode requests and coverage concern only angle 3's media (and the chosen audio's); the other eight have none
- **TEST** t3506 (`src/libs/kt-engine/tests/multicam.rs`)

#### Scenario: A live pass is one undo step and keeps the sound
- **WHEN** during playback digits 2, 1, 3 are pressed at three moments, Stop, then Undo once; and the audio of the cut instance is rendered against the uncut one with the fixed sound
- **THEN** three cuts with angles 2, 1, 3 appear at the displayed frames; one Undo removes all three; the audio is byte-equal to the uncut instance's
- **TEST** t3507 (`src/libs/kt-model/tests/multicam.rs`), t3509 (`src/libs/kt-engine/tests/multicam.rs`)

#### Scenario: Expand keeps what plays
- **WHEN** a piece with angle 2 and a graded angle track is expanded
- **THEN** the piece becomes angle 2's events with the grade and the chosen sound; the picture and sound of the range are equal before and after
- **TEST** t3508 (`src/libs/kt-engine/tests/multicam.rs`)
