## ADDED Requirements

### Requirement: An export renders a chosen format, and all formats in one press
An export SHALL render one target, the master or a named format, at that target's raster through the one plan. The render
window SHALL render every checked target one after another from one press and one file name, the others suffixed by the
format's name; a file that exists SHALL be asked about before any of them starts; Cancel SHALL stop the rest. The command
line SHALL take a format's name, the master, or all.
Why: the owner's ★15 "в рендере «все форматы сразу»".

#### Scenario: All formats from one press
- **WHEN** the master, the vertical and the square are rendered over a region, then again with one of the files present, then cancelled during the second
- **THEN** three files with their rasters and suffixes; each file's frames equal the composites of its format; their audio is byte-equal; the second start asks once before anything is written; the cancel leaves the first file whole and no third
- **TEST** t3488 (`src/libs/kt-engine/tests/frame_formats.rs`), t4238 (`src/apps/kt-app/src/layout_tests.rs`)
