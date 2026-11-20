## ADDED Requirements

### Requirement: A frame format is a raster and per-event framing of the one edit
The one plan SHALL take a render target — the master raster or one of the project's formats — and build
the frame from the events the one event door gives for that target: in a format, an event's Geometry
node values and curves SHALL be its override for that format where one exists, events hidden in the
format SHALL be absent, and an event whose rule follows the project SHALL resolve it by the format's
rule; every other event SHALL keep its master geometry, placed in the format's raster by the same
fractions. Cuts, effects, transitions, morpher layers and sound SHALL be the same in every format. The
caches SHALL key a composite by its target's raster and framing.
Why: invariant 7 — one plan; F-PRJ-006 — a copy per format diverges.

#### Scenario: A format without overrides re-conforms every event
- **WHEN** a 1920×1080 project with a full-frame 16:9 shot, a picture-in-picture at the top right and a title is planned for a 1080×1920 format with rule fill and no overrides
- **THEN** the shot fills the vertical from its centre, the PiP sits at the top right by the same fractions, the title is laid out for 1080 px width; the master plan is unchanged
- **TEST** t3485 (`src/libs/kt-engine/tests/frame_formats.rs`)

#### Scenario: An override and a hidden event act in their format only
- **WHEN** an event gets an override (pos, zoom) in the vertical and another event is hidden in the square
- **THEN** the vertical's plan uses the override, the master's and the square's do not; the square has no layer of the hidden event and the master and vertical do; its linked audio sounds in every format
- **TEST** t3486 (`src/libs/kt-engine/tests/frame_formats.rs`)

#### Scenario: Switching formats never shows another format's frame
- **WHEN** the preview composes a moment in the master, switches to the vertical, back, and a disk-tier entry exists for the master moment
- **THEN** each switch composes or restores the frame of the shown format; no memo or disk entry of one format is shown for another; the audio is not rebuilt by a switch
- **TEST** t3487 (`src/libs/kt-engine/tests/frame_formats.rs`)
