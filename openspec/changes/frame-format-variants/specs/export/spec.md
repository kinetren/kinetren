## ADDED Requirements

### Requirement: An export renders a chosen format, and all formats in one press
An export SHALL render one target — the master or a named format — at that target's raster through the
one plan; the render window SHALL queue one export per checked format, each file suffixed by the
format's name, with the shared settings; the command line SHALL accept a format name or all.
Why: the owner's ★15 "в рендере «все форматы сразу»".

#### Scenario: All formats from one press
- **WHEN** the master, the vertical and the square are checked and rendered over a region
- **THEN** three files are written with their rasters and suffixes; each file's frames equal the preview composites of its format at the same moments; their audio is byte-equal
- **TEST** t3488 (`src/libs/kt-engine/tests/frame_formats.rs`)
