## ADDED Requirements

### Requirement: Every effect chain is edited in one node editor that fits its window
The FX window (an event and its link partners), the track window, the morpher window and a
composition instance's chain SHALL show and edit effects through ONE node editor in one style. It
SHALL open framed (every node in view) and SHALL never scroll: Home frames all, the wheel zooms at
the pointer, a middle drag pans, and below a readable zoom a node shows only its header. Picture and
sound SHALL be two socket kinds and no wire SHALL join them. A simulated device SHALL be one node
with a picture lane and a sound lane, each with its own switch. A morpher layer, a composition and a
track stage SHALL be group nodes that Tab enters and leaves. A node SHALL show at most three main
sliders per lane, named by the plugin's passport, and its height SHALL NOT grow with the passport;
selecting a node SHALL open that slot's existing form beside the graph. Nodes SHALL be laid out by
processing order; dragging a node SHALL change only its order. An effect SHALL be added by dragging a wire into empty canvas or
Shift+A (the existing picker opens at the pointer) and inserted by dropping a node on a wire; M
SHALL mute, X SHALL remove (a device: both halves). Every gesture SHALL be one undo step. A gesture
the engine cannot execute yet (a branch, a second input, a cycle) SHALL be refused with a reason in
the status line, changing nothing. No layout SHALL be stored and no window SHALL be added.
Why: kkt-26.11.18b item 3 — "компактного удобного графа"; U7 — "Вместо полоски", "везде в окнах
разных единым стилем в одном месте чтобы не скролить … типо как в blender"; R.63 — no new window;
rule 17 — one widget. U8 — "2-3 ползунка, но … как уместить огромные эффекты типо isp".

#### Scenario: An event and its linked audio in one node editor
- **PATH** double click a video event linked to its audio → the FX window shows the node editor framed: Source, Geometry, Color Curves, Glow, Output on the picture wire, EQ on the sound wire → drag a wire from Glow's picture output into empty canvas → the picker gallery opens at the pointer → click VHS Tape → a VHS Tape node with a picture lane and a sound lane is inserted after Glow and EQ → its deck face opens beside the graph
- **WHEN** the person adds VHS Tape through the node editor of a linked pair
- **THEN** the video chain ends with `vhs_tape` after Glow, the audio chain with `vhs_tape_sound`; the geometry dump names the nodes, the lanes and the selected node; one undo step removes both
- **TEST** t2994 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Insert on a wire, refuse a branch, mute and remove a device
- **PATH** drag the Glow node onto the wire between Geometry and VHS Tape → the wire highlights → release → Glow moves there → drag a wire from Glow's output onto a second input → it snaps back and the status line says branches are not executed yet → select VHS Tape → press M → both lanes dim → press X → the device goes with both halves → Ctrl+Z → it returns
- **WHEN** the person reorders by dropping on a wire, tries a branch, mutes and removes a device
- **THEN** the picture chain is reordered in one step; the branch changes nothing in the model; M sets both halves' bypass; the removal takes both halves with their curves and undo brings them back with their values
- **TEST** t2995 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Layers, the track stage and the key are group nodes in the processing order
- **PATH** double click an event under two morpher layers on a track whose compressor is keyed by the music track → the editor shows the event's nodes, then the lower layer's purple group, the upper's, then the Track group with a dashed key wire from "Key: Music" → press Tab on the lower layer → the breadcrumb reads "Event › Layer", its nodes greyed → Tab → back → double click it → its morpher window opens
- **WHEN** the FX window opens on an event reached by layers and a keyed track stage
- **THEN** the groups stand in the engine's order (event, layers bottom-up, track), their nodes are not editable there, and the double click opens the existing morpher window, no other
- **TEST** t2996 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A long chain fits without scrolling
- **PATH** open the FX window on an event with 24 effects linked to three audio events → every node is in view as a header chip → wheel over VHS Tape → the view zooms there and its lanes show → press Home → everything fits again → the window shows no scrollbar over the graph
- **WHEN** a chain is longer than the graph area and the event has several partners
- **THEN** the opened view frames every node; zooming out past readability collapses nodes to headers; Home restores the fit; each audio partner is its own sound wire; no node label wraps
- **TEST** t3001 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: A huge device stays a small node with its main sliders
- **PATH** add Camera ISP to an event with linked audio → the ISP node shows its look chip, two picture sliders and one sound slider, no more → drag the exposure slider in the node → the preview brightens → click the node's title → the full ISP panel opens beside the graph with all its blocks
- **WHEN** a device with tens of rows is added and its main slider is dragged in the node
- **THEN** the node has at most three rows per lane and the same height whatever the passport holds; the drag sets that row in the MODEL in one undo step, exactly as the same row in the side form; the side form shows the whole device
- **TEST** t2999 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The Viewer shows the picture after a node
- **PATH** Ctrl+Shift+click the Geometry node of an event carrying Geometry, VHS Tape and Glow → the preview shows the picture after Geometry with the badge "Viewer: Geometry" → Esc → the full picture returns
- **WHEN** the person views the output of a node in the middle of a chain
- **THEN** the preview's frame equals the frame of the same event whose chain ends at that node, built by the one plan; Esc returns the frame of the whole chain
- **TEST** t2997 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: The node editor costs nothing at rest
- **PATH** open the FX window and the morpher window on the heavy showcase project carrying VHS Tape, Datamosh and Camera ISP → move the mouse over a node and leave it still → only the hover tint changes → wait → no frame is asked
- **WHEN** the editor is open and nothing moves
- **THEN** the work counters show no layout of the editor on a hover frame and no frame at rest; the live idle run with both windows open stays at its rest load
- **TEST** t3003 (`src/apps/kt-app/src/idle_tests.rs`), t3006 live `idle` (`src/scripts/live/idle.py`)
