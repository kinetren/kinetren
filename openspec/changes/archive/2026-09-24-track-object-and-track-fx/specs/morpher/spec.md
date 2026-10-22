# Spec Delta

## MODIFIED Requirements

### Requirement: Target mode — a layer of "everything below" by rows
The default of the mode of targets SHALL be "everything below the morpher row"; "below" is counted by the ORDER OF ROWS, and the order of rows SHALL be a state of the model — the order of the vector of track objects (goes through undo, raises the dirtiness of the content); morph and camera tracks are the same objects without level and chain. The order of stacking layers SHALL be bottom to top (the bottom layer is applied first), within one row — by id. The morpher remains per-clip and is applied BEFORE the track stage (the chain, level, and blend mode of the track see the already changed clips). The frozen list of targets MUST be non-empty; an empty set of targets for a layer is legitimate.
Why: the user asked "not to suffer with selection"; reordering rows changes the picture — so it is editing, not a UI convenience; the order as the vector of tracks removes a second source of truth (`lane_order` next to the counters).

#### Scenario: The layer covers what is beneath it, bottom to top
- **WHEN** a morph row stands at the bottom of the list, then is raised to the top; two layers on different rows
- **THEN** the bottom layer covers nothing, the top one covers everything below it; a video layer does not touch audio; the bottom layer is applied first; reordering rows reverses the order of effects and is rolled back by undo
- **TEST** t235 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The order of rows is model state (container, capability `ui-timeline`)
- **WHEN** rows are reordered by a command
- **THEN** the order survives undo/redo, insertion/removal of a track shifts it inside the command, garbage is normalized; in the file the order is the order of the tracks, there is no separate field
- **TEST** t241, t49 (`src/libs/kt-model/tests/lane_order.rs`), t241 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The morpher window shows the mode and the time
- **WHEN** the morpher window is open in layer mode and in frozen-list mode
- **THEN** in layer mode an explanation of what is covered is shown; the frozen list — by name; the checkbox "by time" edits the model in one history step
- **TEST** t242 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The morpher before the track stage
- **WHEN** a morph layer paints the clips of a track that has a track chain with a vignette
- **THEN** the vignette lies on the already painted composite of the track; the order "morpher → track" does not depend on the position of the morph row relative to this track higher in the list
- **TEST** t736 (`src/libs/kt-engine/tests/track_video.rs`)
