# Spec Delta

## ADDED Requirements

### Requirement: The wave W5 transitions
`iris` SHALL offer heart, star and hexagon after its three shapes; `lens_warp`, `ripple_warp` and `twirl_warp` SHALL be
built-in kinds that equal the outgoing side at the start and the incoming at the end byte for byte and are their own
inversion; `push` and `slide` SHALL blur along their motion by a row whose default 0 gives the former bytes.
Why: the owner's K7 (transitions half of the complaint); R.51's inversion rule.

#### Scenario: Shapes and warps end as the sides
- **WHEN** each of the three shapes and three warps renders at k = 0, 2048 and 4096 on two different frames, and inverted
- **THEN** the ends are the two frames byte for byte, the middle is neither, the inverted render is the mirror of the plain
- **TEST** t4275 (`src/libs/kt-gpu/tests/trans_iris_shapes.rs`), t4276 (`src/libs/kt-gpu/tests/trans_warps.rs`)

#### Scenario: The motion blur is off by default
- **WHEN** push and slide render at k = 2048 with the row absent, at 0 and at 0.5 in each direction
- **THEN** the first two equal the former bytes; the third differs only along the direction
- **TEST** t4277 (`src/libs/kt-gpu/tests/trans_push_blur.rs`)

#### Scenario: The picker offers them
- **WHEN** the person opens a transition's window on a crossfade, picks "Twirl", sees the strip, and undoes
- **THEN** the crossfade's kind is the warp, the preview shows it, one undo returns the crossfade
- **PATH** double-click a crossfade → its window shows the kinds; click "Twirl" in the Styled group → the strip and the preview show it twisting; Ctrl+Z → the crossfade is as before
- **TEST** t4278 (`src/apps/kt-app/src/reach_tests.rs`)
