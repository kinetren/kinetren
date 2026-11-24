## ADDED Requirements

### Requirement: The node editor makes branches and merges
The node editor SHALL draw a chain's arms as parallel rows under its trunk, laid out by processing
order with nothing stored, and SHALL make them by gestures: a wire dragged from a node's output into
empty canvas SHALL open the picker there and start a new arm from that output with the picked
effect; the picker's first row SHALL be Merge; a wire dropped on a Merge's Foreground or Matte socket
SHALL connect it, and a wire dragged off a socket SHALL disconnect it, the node then drawn dim as not
reaching the Output. Each gesture SHALL be one undo step, its slots, wires and slot masks one command batch. A picked effect SHALL go after the
selected node of a lane that branches, else at the end; a lane that does not branch SHALL keep the
empty wiring through every gesture. A time kind (Slowed, Datamosh, the pitch's
edge reading) in an arm SHALL be accepted and its node marked as a second read. A wire
that would close a loop, the leading Geometry moved into an arm, a branch past `MAX_HELD`, a read
past `MAX_READS` and a wire across lanes SHALL be refused as an
error in the status line, changing nothing. This supersedes the refusal of "a branch, a second input, a cycle" in the editor's
requirement (fx-graph), except the cycle.
Why: U7 — "типо как в blender"; v1 promised branches and refused them honestly until the engine
executes them.

#### Scenario: A branch, a Merge, a refusal
- **PATH** double click an event carrying a Blur → its node editor shows In → Geometry → Blur → Out → drag a wire from Geometry's output to the empty space below → the picker opens there → pick Vignette → a second row appears under the trunk with Vignette wired from Geometry, dim → drag from Blur's output (or Geometry's) to the empty space → pick Merge, the first row → Merge stands before Out with Blur in Background (and Geometry in Foreground) → drag a wire from Vignette's output onto Merge's Matte socket → it connects and the preview changes where the vignette is bright → click Vignette and pick Slowed+Reverb → it stands after Vignette in the arm marked "Reads the source again" and the event's coverage line shows a second row → drag the Geometry node onto Slowed → the status line says it stays before the first branch, nothing changes
- **WHEN** the person makes an arm, a Merge and a matte wire, puts Slowed in the arm, then tries the lead in the arm
- **THEN** the model's chain has the arm, the Merge with its inputs and Slowed in the arm in four undo steps; the window's Cancel gives the line back; the refusal changes nothing and names the reason
- **TEST** t3218, t3219, t3220 (`src/apps/kt-app/src/node_editor_tests.rs`), t3218b (`src/apps/kt-app/src/graph_layout.rs`), t3296, t3297 (`src/libs/kt-model/tests/fx_strip.rs`), live `node_graph_branch`

#### Scenario: A read is seen and a cell stands where the click puts it
- **PATH** put Slowed in an arm of an event → a second thin row stands over the event's coverage line on the timeline → open Diagnostics → the performance section says "Reads ×1: #1 Slowed+Reverb" → select a node of the arm and open the picker → each cell shows the effect after that node
- **WHEN** an arm of an event holds a time kind, and the picker is opened with a node of an arm selected, or from a wire let go on empty space
- **THEN** the event has a coverage row per read in what that read walks, the performance section names the reads, `kt-cli probe` prints the arms and the reads; a cell is the project with the candidate after that node, or on the new arm alone
- **TEST** t3299 (`src/apps/kt-app/src/node_editor_tests.rs`), t3225 (`src/apps/kt-cli/src/main.rs`)

#### Scenario: The Viewer on an arm
- **PATH** in the same graph Ctrl+Shift+click Vignette → the preview shows the arm's own picture with the badge "Viewer: Vignette" → Esc → the full picture returns
- **WHEN** the Viewer is asked on a node of an arm
- **THEN** the player gets the chain cut to that node's ancestors; Esc restores it; the model is unchanged
- **TEST** t3207 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A branched graph costs nothing at rest
- **PATH** open the heavy showcase → double click an event whose graph branches → its effects window shows the arms → the mouse rests, then moves over the nodes → nothing redraws by itself
- **WHEN** the FX window shows a branched graph on the heavy showcase, on a pause, with the mouse moving over it
- **THEN** no frame is asked at rest; a hover lays nothing out (the layout is kept per wiring); the work counters of the idle check stay at their rest values
- **TEST** t3221 (`src/apps/kt-app/src/idle_tests.rs`)
