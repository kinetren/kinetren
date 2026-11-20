## ADDED Requirements

### Requirement: The 3D switch and extrusion are written by content
A track's 3D switch and a mask's extrusion SHALL raise the `.kt` version by content only when set; a project without
them SHALL be written byte for byte as before; an older build SHALL refuse a file with them naming the version.
Why: an older build would flatten the group on save.

#### Scenario: Round trip and refusal
- **WHEN** a project with a 3D group and an extruded mask is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a project without them is byte-equal to before
- **TEST** t3956 (`src/libs/kt-model/tests/group3d.rs`)
