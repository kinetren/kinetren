## ADDED Requirements

### Requirement: Simulated poses enter the plan as numbers
When a layer's owner has element sets, the frame plan SHALL carry each element's placed pose for the moment as numbers
— vertices for the mask raster, transforms for reel layers and 3D planes, an instance buffer for a title's letters and
an emitter's particles. The plan key and the durable key SHALL hash what those numbers are a function of — the sets and
their drivers up to the moment, and the moment — never a computation's history, and a probe plan SHALL have the full
plan's key without stepping. Masks and transforms SHALL be drawn by their own paths; letters and particles by instanced
passes warmed with the project: a title's letters as masks combined by layer, so that its shadows, strokes and fills lie
in the title's own order.
Why: invariant 4 — the memo and the disk frames stay exact; invariant 7 — one composition path; R.59 — physics gives
numbers, not pixels.

#### Scenario: The memo and the disk frames stay exact under simulation
- **WHEN** a frame with a simulated title is composed, kept in the memo and as a disk frame, then the brake curve that drives it is changed after that moment, then before it
- **THEN** the change after the moment keeps the key and the kept frame is read; the change before it changes the poses, the key and the picture; a fresh compose equals the kept frame where the key is kept
- **TEST** t3987 (`src/libs/kt-engine/tests/simulated_elements.rs`)

#### Scenario: A simulated title's letters are placed by the plan, from the source's atlas
- **WHEN** one reel holding a title with a spring set is placed twice at two outer speeds, and the same title without a set is composed beside it
- **THEN** the two instances read one glyph atlas from the source and give two plans with two keys and two pictures; at rest poses the sprite title, with plate, a widely blurred shadow, stroke and fill over overlapping letters, equals the source's raster of the title without a set within the stored frames' tolerance; a turned letter's shadow keeps its direction; after a text edit no frame draws the new letters from the old atlas; the title without a set keeps its plan key and bytes
- **TEST** t4107 (`src/libs/kt-engine/tests/simulated_elements.rs`)
