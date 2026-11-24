## ADDED Requirements

### Requirement: An added effect's preset is changed in place
Every effect slot's form SHALL begin with a Preset menu listing the kind's variants exactly as the picker lists them
and the user's single-effect presets of that kind; choosing one SHALL replace the effect in one undo step, keeping
the slot's position, bypass, masks and identity; the menu SHALL show the preset the effect currently equals, or
"(custom)" after any edit.
Why: the owner, 2026-11-07: "невозможно после добавления эффекта - ПОМЕНЯТЬ ЕГО ПРЕСЕТ НЕ ПЕРЕСОЗДАВАЯ ЕГО".

#### Scenario: From Heavy to Soft without re-creating
- **WHEN** a Blur added as "Heavy" in the middle of a three-slot chain with a mask is switched to "Soft" from its Preset menu, a value is then nudged, then Undo twice
- **THEN** the slot stays second with its mask and bypass, its values equal the picker's "Soft"; the menu reads "Soft", then "(custom)" after the nudge; two Undos return to "Heavy"; an audio reverb switched the same way keeps ringing (its state object is the same)
- **PATH** double-click the event → effects window → select the Blur node → the form head shows "Preset: Heavy" → open it → "Soft" → the picture softens, the node stays in place → drag the radius a little → the head reads "(custom)" → Ctrl+Z twice → "Heavy"
- **TEST** t3595 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The preset of a layer's, a track's and a device's effect
- **WHEN** the preset of an effect of a morpher layer, of a track's chain (with a user preset that carries curves in the list) and of a VHS Tape on a linked pair is changed
- **THEN** each is one undo step of its owner; on the track the preset with curves stands disabled with its reason; both halves of the device take the preset
- **PATH** the track window → its chain → a node → the form head "Preset: …" → the list: the preset with curves is grey, its hint says why → another: the form changes → Ctrl+Z: it is back
- **TEST** t4243 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A chain preset joins the chain as a selected group, and several nodes are handled with the mouse
Inserting a chain preset SHALL add its nodes where the picker adds an effect (after the selected node on a lane that
branches, else at the lane's end) without removing any existing node, and SHALL leave the inserted nodes selected; Ctrl+click SHALL add or remove a node from
the selection, a drag from the empty canvas SHALL select the nodes its frame touches, Delete SHALL remove every
selected node in one undo step, and dragging a selected node SHALL move the whole selection in one step. A pasted
chain block SHALL be inserted the same way.
Why: the owner, 2026-11-20 (A4) — "к существующей цепочке на графе соединить ту что с пресета или мышкой быстро
выделить и удалить лишнее".

#### Scenario: Insert a chain preset, drop two of its nodes
- **WHEN** a five-node chain preset is inserted into a chain of three nodes, then into a graph with a branch whose arm's node is selected; two of the new nodes are Ctrl+clicked out of the selection; Delete; then Undo
- **THEN** the five nodes stand at the end of the line, and after the selected node of the arm, selected; after the clicks three stay selected; Delete removes those three in one step and the other nodes and wires are untouched; Undo returns them
- **PATH** double-click the event → effects window → picker → "Presets" → a five-node chain → five highlighted nodes appear at the end, the status line says "5 nodes added" → Ctrl+click two of them → they lose the highlight → Delete → three nodes vanish → Ctrl+Z → they are back
- **TEST** t3932 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A selection frame and a group move
- **WHEN** a drag on the empty canvas frames nodes 2–4 of a six-node chain, then node 3 is dragged past node 6
- **THEN** nodes 2–4 are selected; after the drag the chain reads 1, 5, 6, 2, 3, 4 in one undo step, curves, masks and switches with their slots
- **PATH** in the effects window drag on the empty canvas around nodes 2–4 → they highlight → drag node 3 to the right of node 6 → the three move together → Ctrl+Z → the order returns
- **TEST** t3933 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A group on two arms is not moved in half
- **WHEN** nodes of two arms are selected with Ctrl+click and one is dragged; then Delete; a Ctrl+Shift+click and a wire drag from a socket are made with a group selected
- **THEN** the drag is refused in words and nothing moves; Delete removes them all in one step and the wires heal as one removal does; the Viewer and the wire act on their one node
- **PATH** in a graph with a branch Ctrl+click a node on each arm → drag one: the status line says nodes of two branches do not move together → Delete: both vanish, the wires close → Ctrl+Z: both are back
- **TEST** t4244 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Paste joins the same way
- **WHEN** a two-node chain block is copied and pasted on a graph with a selected node
- **THEN** the two nodes stand where the picker would add, selected
- **PATH** select two nodes → Ctrl+C → click a node in another event's graph → Ctrl+V → two highlighted nodes at the end of the chain
- **TEST** t3934 (`src/apps/kt-app/src/reach_tests.rs`)
