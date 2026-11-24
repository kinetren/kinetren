## ADDED Requirements

### Requirement: Simulated poses enter the plan as numbers in its key
When a layer's owner has element sets, the frame plan SHALL carry each element's placed pose for the moment as numbers
— glyph poses for the title's rasterization, vertices for the mask raster, transforms for reel layers and 3D planes, an
instance buffer for particles — and the plan key and the durable key SHALL hash those numbers, never a history or the
moment a computation started. The elements SHALL be drawn by their producer's own drawing path; particles by one
instanced-sprite pass warmed with the project.
Why: invariant 4 — the memo and the disk frames stay exact; invariant 7 — one composition path; R.59 — physics gives
numbers, not pixels.

#### Scenario: The memo and the disk frames stay exact under simulation
- **WHEN** a frame with a simulated title is composed, kept in the memo and as a disk frame, then the brake curve that drives it is changed after that moment, then before it
- **THEN** the change after the moment keeps the key and the kept frame is read; the change before it changes the poses, the key and the picture; a fresh compose equals the kept frame where the key is kept
- **TEST** t3987 (`src/libs/kt-engine/tests/simulated_elements.rs`)
