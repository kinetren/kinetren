# Spec Delta

## ADDED Requirements

### Requirement: Droste and relief are pure kernels
`droste` SHALL be a warp plugin whose phase is periodic and whose neutral values add no step; `relief` SHALL shade the picture
from the luma's slope by one light, neutral at depth 0 in its colour mode; both SHALL be held to stored frames.
Why: the owner's K7 (F-FX-005); a warp is an inverse map, a plugin's own arm needs no core edit.

#### Scenario: The spiral repeats
- **WHEN** `droste` renders at phase 0 and one full period, and at its neutral values
- **THEN** the first two agree within `FrameTol::WARP`, the last is the input byte for byte
- **TEST** t4287 (`src/libs/kt-gpu/tests/droste.rs`)

#### Scenario: The light decides the shading
- **WHEN** `relief` runs on a ramp with the light left, right and depth 0
- **THEN** the left and right results are mirrored, depth 0 in the colour mode is the input
- **TEST** t4288 (`src/libs/kt-gpu/tests/relief.rs`)

### Requirement: Trails blend the picture with its own past frames
`trails` SHALL blend N ≤ 7 earlier input frames of its slot, `step` project frames apart, with levels that decay by a row;
the frames SHALL be the ones the plan's order asks, so export, preview and the coverage bars agree; N = 0 SHALL be the line
byte for byte; more past frames than the layer's bound and a reel instance SHALL be refused with the reason.
Why: the owner's "buffer + blend levels"; a window of inputs is a function of the moment, so a seek is free.

#### Scenario: The past is asked and is the same on a seek
- **WHEN** trails of 3 frames step 2 renders frame 100 cold after frames 94–99, and after a seek to 100
- **THEN** both are byte-identical, `wanted_frames(100)` lists the frames 94, 96, 98 and 100, N = 0 equals the line, at the event's start the first frame stands in
- **TEST** t4290 (`src/libs/kt-engine/tests/trails.rs`)

#### Scenario: A person adds trails
- **WHEN** they add Trails to an event and drag Count to 4
- **THEN** the preview shows the ghosts and playing it keeps what it can
- **PATH** "+ Effect" → type "trails" → click the cell → a node "Trails" with Count and Decay; drag Count to 4 → ghosts follow the motion; Count 0 → the picture is plain; Ctrl+Z twice → the node is gone
- **TEST** t4291 (`src/apps/kt-app/src/reach_tests.rs`)
