## ADDED Requirements

### Requirement: Neighbouring 3D tracks are drawn as one group with depth
A run of neighbouring video tracks switched to 3D SHALL be planned as one group: each member's picture after its own
chain becomes the texture of a mesh (a plane by its transform, or an extruded mask), the group SHALL be drawn by the
active camera with a depth buffer so members intersect and sort per pixel by depth, and its image SHALL enter the 2D
stack at the group's place; a 2D track between 3D tracks SHALL split them; preview and export SHALL draw it in the one
plan, within the GPU composite's tolerance.
Why: the owner, v26.10 E1 — "настоящие 3d-меши с перекрытием … не потеряв в производительности".

#### Scenario: Two planes cross along a line
- **WHEN** two 3D tracks hold a red and a blue plane rotated ±30° around Y through the same centre, rendered by the default camera; then a 2D track is put between them
- **THEN** each half of the frame shows the nearer colour, the boundary is the projected intersection line within 1 px; with the 2D track between them the upper track covers the lower entirely
- **TEST** t3952 (`src/libs/kt-gpu/tests/group3d.rs`), t3953 (`src/libs/kt-engine/tests/group3d_plan.rs`)

#### Scenario: Extruded parallax pieces occlude as the camera moves
- **WHEN** a photo split into three extruded mask pieces at depths 0, 200 and 400 px is rendered as a camera pans 300 px
- **THEN** the near piece covers the far ones where they overlap at each camera position, the pieces' sides are visible at the edges, and the export equals the preview within `FrameTol::BILINEAR`
- **TEST** t3954 (`src/libs/kt-gpu/tests/group3d.rs`)
