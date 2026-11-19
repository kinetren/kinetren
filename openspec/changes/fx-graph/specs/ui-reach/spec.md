## ADDED Requirements

### Requirement: Every effect chain is edited in one node editor that fits its window
The FX window (an event and its link partners), the track window, the morpher window and a
composition instance's chain SHALL show and edit effects through ONE node editor in one style. It
SHALL open framed (every node in view) and SHALL never scroll: the detailed nodes when they fit
the width, else small chips wrapped into bands (the zoom is a LEVEL OF DETAIL — the text stays
sharp and every widget keeps its real place); Ctrl+wheel over the graph switches detail and
chips, a middle drag pans the detail, "Frame all" and a double click on empty space return the
fit; the plain wheel stays the window's scroll. Picture and
sound SHALL be two socket kinds and no wire SHALL join them. A simulated device SHALL be one node
with a picture lane and a sound lane, each with its own switch. After an event's own nodes the morpher layers
over it (bottom up, the render's rule) and its track's stage SHALL stand as read-only boxes; a
double click on one SHALL open its own window. A sidechain key SHALL be drawn as a box wired into
its compressor. A node SHALL show at most three main
sliders per lane, named by the plugin's passport, and its height SHALL NOT grow with the passport;
selecting a node SHALL open that slot's existing form beside the graph. Nodes SHALL be laid out by
processing order; dragging a node SHALL change only its order. An effect SHALL be added by dragging a wire into empty canvas or
the picker; a drag SHALL reorder within its lane by dropping a node on another; every node SHALL
carry its switch and its cross, and a device's cross SHALL remove both halves. While the pointer is over a graph,
the key table's own commands SHALL act on it: Delete on the selected node, the mute key on its
switch, zoom to fit as Frame all, zoom in/out as detail/chips — no new key, the timeline does not
get them. Every gesture SHALL be one undo step. A gesture
the engine cannot execute yet (a branch, a second input, a cycle) SHALL be refused with a reason in
the status line, changing nothing. No layout SHALL be stored and no window SHALL be added.
Why: kkt-26.11.18b item 3 — "компактного удобного графа"; U7 — "Вместо полоски", "везде в окнах
разных единым стилем в одном месте чтобы не скролить … типо как в blender"; R.63 — no new window;
rule 17 — one widget. U8 — "2-3 ползунка, но … как уместить огромные эффекты типо isp".

#### Scenario: A device's halves stand in one column
- **PATH** double click a morpher block carrying a Blur, VHS Tape and a Gain → the morpher window shows its node editor: the purple source node, Blur and VHS Tape on the picture lane, Gain and VHS Tape's sound (with the speaker) on the sound lane, the tape's two halves one above the other in one frame
- **WHEN** the morpher window opens on a layer carrying a device and other effects of both kinds
- **THEN** the device's halves share one column, the picture half above; Blur stays on the picture lane and Gain on the sound lane, each in its chain's order; the graph publishes its place (an event and its linked audio in ONE editor follows in v1c)
- **TEST** t2994 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Reorder within a lane, refuse across, remove a device
- **PATH** in the morpher's node editor drag Blur onto VHS Tape → Blur stands after the tape → drag Gain onto VHS Tape (another lane) → the status line says an effect stays on its lane, nothing changes → click the cross of VHS Tape's picture half → both halves go
- **WHEN** the person reorders by dropping a node on another, tries the other lane, and removes a device
- **THEN** the picture chain is reordered in one step; the drop across lanes changes nothing in the model; every node has its cross, and the device's cross takes both halves
- **TEST** t2995 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Layers, the track stage and the key are group nodes in the processing order
- **PATH** double click an audio event under two morpher layers on a track whose compressor is keyed by the music track → its effects window shows the event's nodes, then two purple layer boxes, then the track's box with "Key: …" → double click the first layer box → that morpher's window opens → open the track's window → the key box stands under its compressor, wired into it
- **WHEN** the effects window opens on an event reached by layers and a keyed track stage
- **THEN** the boxes stand in the render's order (the event's nodes, the layers bottom up, the track), they edit nothing there, the track's box names its key, and a double click opens the existing window of the box, no other; the track window draws the key box with a dashed wire into its compressor
- **TEST** t2996 (`src/apps/kt-app/src/node_editor_tests.rs`), live `node_graph` (`src/scripts/live/node_graph.py`)

#### Scenario: A long chain fits without scrolling
- **PATH** open the FX window on an event with 24 effects → every node is in view as a chip → Ctrl+wheel over the graph → the detailed nodes with their sliders, a middle drag pans them → click "Frame all" → all the chips again, no scrollbar over the graph
- **WHEN** a chain is longer than the graph area
- **THEN** the opened view frames every node inside the window's width; Ctrl+wheel opens the detailed nodes; "Frame all" restores the fit; no node label wraps
- **TEST** t3001 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A huge device stays a small node with its main sliders
- **PATH** add Camera ISP to an event → the ISP node shows its exposure, gain and sharpening sliders, no more → drag the exposure slider in the node → the preview brightens → the full ISP panel stays below the graph with all its blocks
- **WHEN** a device with tens of rows is added and its main slider is dragged in the node
- **THEN** the node has at most three rows per lane and the same height whatever the passport holds; the drag sets that row in the MODEL in one undo step, exactly as the same row in the side form; the side form shows the whole device
- **TEST** t2999 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The Viewer shows the picture after a node
- **PATH** Ctrl+Shift+click the Geometry node of an event carrying Geometry, VHS Tape and Glow → the preview shows the picture after Geometry with the badge "Viewer: Geometry" → Esc → the full picture returns
- **WHEN** the person views the output of a node in the middle of a chain
- **THEN** the preview's frame equals the frame of the same event whose chain ends at that node, built by the one plan; Esc returns the frame of the whole chain
- **TEST** t2997 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The table's keys act on the graph under the pointer
- **PATH** in an event's effects window click the Blur node → with the pointer over the graph press the mute key → Blur is struck through → press Delete → Blur goes, the event stays on the timeline → press zoom in → the detailed nodes → press zoom to fit → the fit again
- **WHEN** the person presses the key table's mute, Delete and zoom keys with the pointer over the graph
- **THEN** the selected node's bypass is switched and then the node removed in the MODEL, the timeline keeps its event and its tracks' switches; zoom in and zoom to fit change the graph's view (in a style that binds them)
- **TEST** t3002 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A chain of sound draws its nodes
- **PATH** double click an audio event carrying Gain and Pitch Shift → its effects window shows both as nodes between In and Out
- **WHEN** the effects window opens on an audio event (or an audio track's window on its chain)
- **THEN** every slot is a node; a single-kind chain is one lane whatever its kind (found-defects 811)
- **TEST** t3000 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The node editor costs nothing at rest
- **PATH** open the FX window and the morpher window on the heavy showcase project carrying VHS Tape, Datamosh and Camera ISP → move the mouse over a node and leave it still → only the hover tint changes → wait → no frame is asked
- **WHEN** the editor is open and nothing moves
- **THEN** the work counters show no layout of the editor on a hover frame and no frame at rest; the live idle run with both windows open stays at its rest load
- **TEST** t3003 (`src/apps/kt-app/src/idle_tests.rs`), t3006 live `idle` (`src/scripts/live/idle.py`)
