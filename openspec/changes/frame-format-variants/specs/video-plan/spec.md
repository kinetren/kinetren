## ADDED Requirements

### Requirement: A frame format is a raster and per-event corrections over the one edit
A format SHALL be composed as the project it derives: the master's edit with the format's raster and rule, in which every
picture event of the root and of the reels without a raster of their own keeps its common geometry with the format's
correction over it — an offset, a zoom factor and a turn added to the base and to every key of the common curves, a rule
or a crop or a row's own curve in place of the common one where given — and events hidden in the format are absent. Every
other event SHALL be placed by the same fractions in the new raster and conformed by its rule. Cuts, effects, transitions,
layers and sound SHALL be the same in every format. A composite SHALL be keyed by the format it was made for.
Why: the owner's ★15 ("у каждого своя поправка геометрии события поверх общей"); A17 ("Каждое по своей геометрии");
invariant 7.

#### Scenario: A format without corrections re-conforms every event
- **WHEN** a 1920×1080 project with a full-frame shot, a picture-in-picture at the top right, a composition without a raster and one with 1280×720 of its own is composed in a 1080×1920 format with rule fill
- **THEN** the shot fills the vertical from its centre, the PiP sits by the same fractions, the first composition's events are conformed one by one, the second is one picture; the master's plan and keys are unchanged
- **TEST** t3485 (`src/libs/kt-engine/tests/frame_formats.rs`)

#### Scenario: A correction lives over the master and acts in its format only
- **WHEN** an event whose zoom and position are curves gets a correction in the vertical, the master's curves are then moved, another event is hidden in the square, and an event linked to the first's position is in the edit
- **THEN** the vertical's placement at every frame is the moved master's plus the correction; the master's and the square's plans have none; the linked event follows the corrected one in the vertical; the hidden one is absent in the square only and its sound is heard in every format
- **TEST** t3486 (`src/libs/kt-engine/tests/frame_formats.rs`), t4235 (`src/libs/kt-model/tests/frame_formats.rs`)

#### Scenario: Switching formats never shows another format's frame
- **WHEN** the preview composes a moment in the master, in 1080×1920 at ½ and in 540×960 at full, with the disk tier on, and back
- **THEN** each shows its own frame, no memo or disk entry of one serves another, and no audio event is rebuilt by a switch
- **TEST** t3487 (`src/libs/kt-engine/tests/frame_formats.rs`)
