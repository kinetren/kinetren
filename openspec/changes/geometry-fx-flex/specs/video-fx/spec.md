## ADDED Requirements

### Requirement: Mirror reflects across a line, and the flip stays for old slots
Mirror SHALL offer a reflect mode: a line through a centre at an angle, the kept side shown as it is and the
other side as its reflection (or the reverse when "both" is set), as an inverse map of the fused geometry
pass; a Mirror slot without the mode row SHALL keep today's flip and bytes; a Mirror added from the picker
SHALL start in reflect mode.
Why: the owner's B6 — "в sony vegas можно назначить угол отражения"; rule 20 for old projects.

#### Scenario: Reflect at angles, seam exact
- **WHEN** a test chart is reflected at 90° through the centre, at 0° through a point at 30 % height, and at 33°
- **THEN** the kept side equals the source pixel for pixel, the other side equals the closed-form reflection within `FrameTol::GEOM`, the seam line has no gap or doubling; the CPU geometry and `geom_fused.wgsl` agree
- **TEST** t3570 (`src/libs/kt-gpu/tests/gpu_render.rs`), t3575 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Old Mirror and EventGrid keep their bytes
- **WHEN** projects with Mirror flips and EventGrids written before this change are composed and saved
- **THEN** the frames equal the stored references (the flip's lone kernel still used alone) and the file text is unchanged
- **TEST** t3571 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: EventGrid flips cells and slides its content; Tile repeats a region with mirrored edges
EventGrid SHALL offer flipping every other column and every other row, a gap between cells, a phase that
slides the content inside the cells and a row shift; a new Tile SHALL repeat a source rectangle of the
picture in tiles of a given size over the canvas, with mirrored edges, a phase and a row shift; both SHALL be
inverse maps of the fused geometry pass with supersampling by their local scale.
Why: the owner's B7–B8 — a grid with flips, a repeat texture that slides.

#### Scenario: Seamless tiles and a sliding phase
- **WHEN** a gradient chart is tiled 4×4 with mirrored edges and its phase_x animated from 0 to 1 over 25 frames; and an EventGrid 3×3 has flip_x and flip_y
- **THEN** neighbouring tiles meet with equal edge pixels (no seam); frame k equals the tile pattern shifted by k/25 of a tile; the grid's odd cells are mirror images of the even ones
- **TEST** t3572 (`src/libs/kt-gpu/tests/gpu_render.rs`), t3573 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The owner's chain in one pass
- **WHEN** an event carries EventGrid 3×3 with flips → Mirror reflect 90° → Mirror reflect 0° → Tile of the top half with mirrored edges and an animated phase, and is composed and exported
- **THEN** the chain runs as one fused geometry pass; its frames equal the four steps run as separate passes within `FrameTol::GEOM` and a CPU closed form of the composed map; preview and export are byte-equal on the device; the stored reference frames hold
- **TEST** t3574 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Reflect's centre and angle are set on the preview
- **WHEN** a Mirror in reflect mode is selected, its centre crosshair is dragged on the preview and its angle handle turned
- **THEN** the reflection line follows on the shown frame and the rows change in one undo step per drag
- **PATH** select the event → effects window → the Mirror slot → a crosshair and a line appear on the preview → drag the crosshair to the subject's face → the reflection line moves there → drag the angle handle → the line turns, the picture reflects at the new angle → Ctrl+Z → the previous angle
- **TEST** t3576 (`src/apps/kt-app/src/reach_tests.rs`)
