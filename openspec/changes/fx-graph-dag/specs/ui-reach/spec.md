## ADDED Requirements

### Requirement: The node editor makes branches and merges
The node editor SHALL draw a chain's arms as parallel rows under its trunk, laid out by processing
order with nothing stored, and SHALL make them by gestures: a wire dragged from a node's output into
empty canvas SHALL open the picker there and start a new arm from that output with the picked
effect; the picker's first row SHALL be Merge; a wire dropped on a Merge's Foreground or Matte socket
SHALL connect it, and a wire dragged off a socket SHALL disconnect it, the node then drawn dim as not
reaching the Output. Each gesture SHALL be one undo step. A wire across lanes, one that would close a
loop, a trunk-only kind (Slowed, Datamosh, the pitch's edge reading, the leading Geometry) moved into
an arm, and a branch past `MAX_HELD` SHALL be refused as an error in the status line, changing
nothing. This supersedes the refusal of "a branch, a second input, a cycle" in the editor's
requirement (fx-graph), except the cycle.
Why: U7 — "типо как в blender"; v1 promised branches and refused them honestly until the engine
executes them.

#### Scenario: A branch, a Merge, a refusal
- **PATH** double click an event carrying a Blur → its node editor shows In → Lead → Blur → Out → drag a wire from the Lead's output to empty canvas below → the picker opens there → pick Luma Key → a second row appears under the trunk with Luma Key wired from the Lead → drag from Blur's output to empty canvas → pick Merge → Merge stands before Out with Blur in Background → drag a wire from Luma Key's output onto Merge's Matte socket → it connects and the preview shows the blur only in the bright areas → drag Slowed+Reverb's node from the picker onto the arm's wire → the status line says Slowed stays before the first branch, nothing changes
- **WHEN** the person makes an arm, a Merge and a matte wire, then tries a trunk-only kind in the arm
- **THEN** the model's chain has the arm and the Merge with its inputs in three undo steps; the refusal changes nothing and names the reason
- **TEST** t3218, t3219, t3220 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The Viewer on an arm
- **PATH** in the same graph Ctrl+Shift+click Luma Key → the preview shows the key's black-and-white picture with the badge "Viewer: Luma Key" → Esc → the full picture returns
- **WHEN** the Viewer is asked on a node of an arm
- **THEN** the player gets the chain cut to that node's ancestors; Esc restores it; the model is unchanged
- **TEST** t3207 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A branched graph costs nothing at rest
- **WHEN** the FX window shows a branched graph on the heavy showcase, on a pause, with the mouse moving over it
- **THEN** no frame is asked at rest; a hover asks no layout; the work counters of the idle check stay at their rest values
- **TEST** t3221 (`src/apps/kt-app/src/idle_tests.rs`)
