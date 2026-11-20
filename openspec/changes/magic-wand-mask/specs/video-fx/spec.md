## ADDED Requirements

### Requirement: A wand mask chooses a region of the source picture per frame
A wand mask SHALL compute its coverage on every frame from the event's decoded source picture at that
frame, its seeds at that frame (mapped from the canvas through the leading Geometry node) and its
parameters: a contiguous region grown from the add seeds, or every pixel within tolerance when not
contiguous, minus the subtract seeds' regions, with an alpha ramp across the soft band, holes filled and
grow/shrink applied when set; the coverage SHALL then join the mask list with the existing modes,
feather, opacity and invert. The computation SHALL use integer and IEEE-exact operations only and SHALL be
memoized by the frame's content, the resolved values and the size; a wand on a track SHALL be refused
with its reason.
Why: invariants 7 and 11 — the one CPU mask rasterizer, the same bytes everywhere; F-MSK-008.

#### Scenario: Contiguous against colour range
- **WHEN** a synthetic frame has two separate blue discs and a blue-green gradient; a seed is put in the left disc, contiguous on, then off
- **THEN** contiguous covers the left disc only; colour range covers both discs; the gradient's coverage ramps across the soft band; a subtract seed in the right disc removes it
- **TEST** t3535 (`src/libs/kt-fx/tests/wand.rs`)

#### Scenario: A seed linked to a track follows the object
- **WHEN** a seed's x/y are linked to a tracker point on a fixture where a red ball moves across a grey wall, and frames 0, 30 and 60 are composed
- **THEN** the coverage covers the ball at each frame within one analysis pixel of its known disc; the wall is uncovered
- **TEST** t3536 (`src/libs/kt-engine/tests/wand_mask.rs`)

#### Scenario: Exact bytes and the memo
- **WHEN** the same wand coverage is computed twice, on Linux and through the Windows test build, and a paused frame is redrawn with the mouse moving
- **THEN** the bytes are identical; the second computation of the paused frame is a memo hit and no new coverage is computed while only the mouse moves
- **TEST** t3537 (`src/libs/kt-fx/tests/wand.rs`)
