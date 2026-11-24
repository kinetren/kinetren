## ADDED Requirements

### Requirement: Mirror reflects across a line, and the flip stays for old slots
Mirror SHALL offer a reflect mode: a line through a centre at an angle on the screen, the kept side shown as it is and the
other side as its reflection, with "keep the other side" and "flip the reflected side along the line", as one map of the
fused geometry pass. A Mirror slot without the mode SHALL keep today's flip and bytes; a Mirror added from the picker
SHALL start in reflect mode.
Why: the owner's B6–B7 ("в sony vegas можно назначить угол отражения", "вертикальное отражение отзеркалить по горизонтали");
A24 ("Старые отражают как раньше").

#### Scenario: Reflect at angles, seam exact
- **WHEN** a test chart is reflected at 90° through the centre, at 0° through a point at 30 % of the width, at 33°, each also swapped and flipped along the line, on a 16:9 and a 9:16 stage
- **THEN** the kept side equals the source pixel for pixel, the other equals the closed-form reflection within `FrameTol::GEOM`, the seam has no gap or doubling, and a circle stays a circle; the CPU map and `geom_fused.wgsl` agree, specialized and interpreted
- **TEST** t3570 (`src/libs/kt-gpu/tests/gpu_render.rs`), t3575 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Old Mirror and EventGrid keep their bytes
- **WHEN** projects with Mirror flips and EventGrids written before this change are composed and saved
- **THEN** the frames equal the stored references (the lone mirror and the exact tile program still used) and the file text is unchanged
- **TEST** t3571 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: EventGrid flips cells and slides its content; Tile repeats a region inside a rectangle
EventGrid SHALL offer flipping every other column and row, a gap, a phase that slides the content inside the cells and a
row shift. A new Tile SHALL repeat a source rectangle of the picture in tiles of a given size inside a destination rectangle
of the canvas, with mirrored edges, a phase and a row shift, and SHALL leave the picture outside that rectangle as it
was. Both SHALL be maps of the fused geometry pass supersampled by their own scale, without blurring at the edges of cells.
Why: the owner's B7–B8 — a grid with flips; a repeated texture "чтобы можно было по ней ездить туда-сюда", made "из верхней части".

#### Scenario: Seamless tiles and a sliding phase
- **WHEN** a gradient chart is tiled 4×4 with mirrored edges and `phase_x` from 0 to 1 over 25 frames, with a row shift; an EventGrid 3×3 has both flips, a gap and a phase over 1
- **THEN** neighbouring tiles meet with equal edge pixels and no blurred line; frame k is the pattern shifted by k/25 of a tile; the odd cells are mirror images of the even ones also where the phase carried the content over; the gap is transparent and the grid stands
- **TEST** t3572 (`src/libs/kt-gpu/tests/gpu_render.rs`), t3573 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The owner's chain in one pass
- **WHEN** an event carries EventGrid 3×3 with flips → Mirror reflect 90° flipped along → Tile of the top half inside the top half with mirrored edges and a phase going there and back, and is composed and exported
- **THEN** the chain is one fused pass; its frames equal the steps run as separate passes and a CPU closed form within `FrameTol::GEOM`, the stored frames hold, preview and export are byte-equal on the device; the bottom half of every frame equals the chain without the Tile byte for byte
- **TEST** t3574 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The file says what it needs
- **WHEN** a project with a reflect, one with grid flips and one with a Tile are saved and given to a reader of the version before, and one with old Mirrors and EventGrids
- **THEN** the first three are written with the new version and refused by the older reader naming it; the last one's bytes do not move
- **TEST** t4245 (`src/libs/kt-model/tests/project_file.rs`)
