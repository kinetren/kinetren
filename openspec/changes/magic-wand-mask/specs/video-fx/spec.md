## ADDED Requirements

### Requirement: A wand mask outlines a region of the source picture per frame
A wand mask SHALL compute closed contours on every frame from the event's decoded source picture at that frame, its seeds
at that frame and its numbers: the region contiguous to the add seeds within the tolerance (or every pixel within it),
minus the subtract seeds' regions, with holes joined when asked, cut where the colour crosses the tolerance between
pixels. The contours SHALL be drawn by the one mask rasterizer with the mask's feather, falloff, expansion, opacity, invert
and mode. The same inputs SHALL give the same bytes on every platform. The colour of a seed SHALL be read from the frame.
Why: invariants 7 and 11 — one rasterizer; F-MSK-008; the owner, kkt-26.11.20 A20 — "самые удобные и точные".

#### Scenario: Contiguous against every matching pixel
- **WHEN** a synthetic frame has two blue discs, one with a hole, and a blue-green gradient; a seed is put in the left disc, then "every matching pixel" is turned on, a subtract seed put in the right disc, "fill holes" turned on
- **THEN** in turn: the left disc alone; both discs; the left one; the hole joins; the contour's radius is within a quarter of an analysis pixel of the disc's
- **TEST** t3535 (`src/libs/kt-fx/tests/wand.rs`)

#### Scenario: A keyed seed follows the object
- **WHEN** a red ball moves across a grey wall whose light dims, a seed has a key at the ball's centre at frames 0, 30 and 60, and the three frames are composed
- **THEN** the coverage covers the ball at each frame within one analysis pixel of its disc, and the wall is uncovered
- **TEST** t3536 (`src/libs/kt-engine/tests/wand_mask.rs`)

#### Scenario: Exact bytes and the memo
- **WHEN** the contours of one frame are computed twice, here and through the Windows test build; a paused frame with a wand is composed, and the mouse moves over the preview
- **THEN** the bits are equal and equal the stored ones; the second composite's contours are a memo hit, and nothing is computed while only the mouse moves
- **TEST** t3537 (`src/libs/kt-fx/tests/wand.rs`)

#### Scenario: A list of shapes and a wand, at two decode steps
- **WHEN** a wand intersects an ellipse with feather 20 and expansion 4 on an event zoomed by its Geometry node, composed with the decode at 1× and at 2×
- **THEN** the intersection is feathered and grown as the ellipse alone would be, the two decode steps agree within two code values away from the contour, and the old shapes' bytes are those of t1269
- **TEST** t3542 (`src/libs/kt-engine/tests/wand_mask.rs`), t3541 (`src/libs/kt-fx/tests/masks.rs`)

### Requirement: A wand reads the source whatever the chain is, and says where it cannot
A wand on an event with a stream stage SHALL read the event's decoded frame, ordered as part of that moment; the moment
SHALL be incomplete until it is there. A wand on a composition instance SHALL be skipped and its row SHALL say why; a wand
on a track SHALL be refused by the model. The key of a moment SHALL name the source's frame, not the process's serials.
Why: the owner's choice, kkt-26.11.20 A20 (Q35) — the source picture; invariant 4.

#### Scenario: Datamosh under a wand, an instance, a track
- **WHEN** a Datamosh event has a wand on its picture and is composed cold and warm; its source is replaced by another file; the same mask list is given to an instance and to a track
- **THEN** the cold moment is incomplete until the frame is decoded and then equals the warm one; the replaced source gives another key and another coverage; the instance composes as without the mask; the track's command is refused naming it
- **TEST** t3543 (`src/libs/kt-engine/tests/wand_mask.rs`)
