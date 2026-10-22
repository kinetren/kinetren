# Spec Delta

## MODIFIED Requirements

### Requirement: Layer geometry — placement rectangle, transparent margins, one function
Each video layer of the plan SHALL carry its placement geometry: a rectangle in FRACTIONS of the canvas and the decode size, computed by ONE pure function from the kind of source (native × pixel aspect), the canvas, the conform rule (fit / fill / stretch / as is), the crop and the zoom. The composite SHALL place the layer into its rectangle; the canvas area outside the rectangle is alpha 0 (transparent margins), the background of the frame is as before. The "fit" rule SHALL preserve the proportions and scale in both directions; "fill" — preserve the proportions and crop; "stretch" — pull to the edges; "as is" — place 1:1 pixels centred, and when the source is larger than the canvas — beyond its edge. The placement SHALL be executed as an affine in the common geometry program with the edge primitive "transparent", a pair of CPU kernel ↔ WGSL by tolerance classes; an affine with a frame equal to the canvas SHALL give the former path byte for byte. An unknown kind of source SHALL be conformed as "stretch" deterministically.
THE SAME function SHALL give the placement rectangle OUTWARD — to the interface that draws the frame over the picture: the rectangle in fractions of the canvas, the rotation angle, and the anchor point in the same coordinates that the composite sees. The inverse recomputation "canvas point → node parameter value" SHALL be its pair in the same place: the shift of the point on the canvas gives the position, the pull of a corner — the zoom, the pull of the rotation handle — the angle, the move of the anchor — the anchor TOGETHER with the position compensation, under which the placement rectangle does not change (at any rotation). A second placement arithmetic MUST NOT exist in the program.
Why: stretching was accepted by the decoder — outside the single code of the composition; a black margin on the top layer erases everything under it (Pitivi #1378); "as is" is the only rule that does not invent pixels. R.64: a screen gesture over the frame must compute exactly the frame that goes into the file, otherwise the control would show one thing and the export would give another.

#### Scenario: Four rules on four sources
- **WHEN** vertical, horizontal, smaller and larger-than-canvas sources (and a source with SAR 2:1) are conformed by each of the four rules
- **THEN** the rectangles equal the analytic ones (fit — margins along the long side, fill — cropping along the short side, stretch — the whole canvas, as is — native centred, beyond the edge on excess); the decode size is even and not above the ceiling; the SAR is taken into account in the view
- **TEST** t712 (`src/libs/kt-fx/tests/conform.rs`)

#### Scenario: Transparent margins on both backends, an identity on matching proportions
- **WHEN** a vertical source is composited into 16:9 on the CPU and on the GPU; a 16:9 source — with the identity affine
- **THEN** the margins are transparent (the lower layer is visible through them), the tolerance class is EXACT at K = 1; the identity affine is byte for byte equal to the former path
- **TEST** t713 (`src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`), t714b (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Goldens do not move
- **WHEN** the golden exports and "preview == export" are run on fixtures with matching proportions
- **THEN** `GOLDEN_VIDEO`, t11, t67, t184 are green without regeneration; a vertical source gives a frame with margins (measurement of the geometry of a white rectangle)
- **TEST** t715 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: The rectangle outward and the inverse computation
- **WHEN** an event has a crop, zoom, position, rotation and anchor set; the interface asks for the placement rectangle; then it asks to recompute the shift of a canvas point into a position, the pull of an angle into zoom, and the move of the anchor
- **THEN** the rectangle equals the one by which the composite places the layer (checked against the frame); the inverse recomputation returns values giving exactly the requested shift; the move of the anchor with compensation leaves the rectangle unchanged at a rotation of 0° and 30°
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`)
