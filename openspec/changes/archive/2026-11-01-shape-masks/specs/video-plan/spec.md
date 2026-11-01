## ADDED Requirements

### Requirement: Masks in the plan — the picture, an effect slot, a track; one raster per application
A mask list SHALL enter the plan as an internal chain slot, materialized by the engine as the leading
"Geometry" node and the morpher's strength are: the event's picture masks as an ALPHA slot right after
the leading Geometry node (the coverage rasterized in the node's output raster with the path mapped
through the node's placement — the one function the composite and the on-screen frame use — so the
mask is "crop → mask → placement" and follows the event's move, zoom and rotation); an effect slot's
masks as a MIX slot right after that slot (the slot's input is snapshotted, the slot runs, the two
are mixed per pixel by the coverage: inside the masks — the effect, outside — its input); a track's
masks as an ALPHA slot after the track's own chain, before its level and blend mode, in canvas space
(a track with masks is a track stage). The masks SHALL be evaluated at the frame's material tick when
the layer is built; the raw masks with their curves SHALL enter the post-FX key and the track stage's
content hash. The planner SHALL treat a mix slot as a strength stop and an alpha slot as a barrier;
neither MAY split a geometry series that has no mask in it. A slot before the leading node is masked
in the source raster, a slot after it in the canvas raster through the placement; a later Geometry
node or a distortion is not followed. The coverage SHALL be computed by the one kt-fx rasterizer once
per frame per application point; the GPU SHALL apply THE SAME coverage bytes in an integer pass, so a
mask adds no divergence of its own: masks and the exact Geometry node agree byte for byte, a masked effect
within that effect's own tolerance class. An event, a slot and a track without masks MUST get no slot: the plan, the
bytes and the cost are the earlier ones.
Why: the owner's S1 ("classic masks") within invariant 7; a canvas mask applied at the composite would
cover different content than a slot that runs before the Geometry node, and a GPU rasterizer would be a
second implementation held to a threshold tolerance (F-MSK-001).

#### Scenario: The picture mask cuts the event and follows its geometry
- **WHEN** an opaque event over a coloured lower track gets a rectangle mask on its left half; then the event's Geometry node moves it right by a quarter of the canvas; then the mask is inverted
- **THEN** inside the mask the event's colour, outside it the lower track's; after the move the visible region moved with the event by exactly the shift; inverted — the halves swap
- **TEST** t1270 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A slot mask limits an effect to the region
- **WHEN** an event with an invert-colour effect gets an ellipse mask on that slot; the same with the mask at 50 % opacity; the same slot placed before the leading Geometry node with the event zoomed
- **THEN** inside the ellipse the pixels equal the effect's, outside — the event's without the effect, byte for byte; at 50 % — the half-way mix; before the node the region is the same content of the source (it scales with the zoom)
- **TEST** t1271 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A track mask cuts the track's composite
- **WHEN** a video track with two events and a transition between them gets a rectangle mask; then the track's level is 0.5
- **THEN** outside the mask the lower layers are untouched, inside — the track's composite (the transition included); the level multiplies only inside; a track without masks keeps the earlier stage condition
- **TEST** t1272 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: Without masks — the earlier plan and bytes; a mask edit is not a stale frame
- **WHEN** a project without masks is composed; then a mask is added and its feather is changed; the frame is composed twice
- **THEN** without masks the chain has no mask slot and the frame equals the earlier path byte for byte (goldens untouched); each mask edit changes the post-FX key, the repeat is read from the cache
- **TEST** t1273 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A mask adds no GPU divergence
- **WHEN** a picture mask after a rotated, zoomed Geometry node (opaque and alpha sources), masks alone on a track's chain, a slot mask before and after the node (a blur, a duotone) and a feathered Difference path are composed on both backends
- **THEN** the mask-only cases agree within `FrameTol::EXACT`; a masked effect within that effect's own class (`BILINEAR`, t181); and every case differs from the same plan without its masks
- **TEST** t1274 (`src/libs/kt-gpu/tests/masks.rs`)
