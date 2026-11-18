## ADDED Requirements

### Requirement: Every effect chain is edited as one compact graph
The FX window, the morpher window and the track window SHALL show and edit their chains through ONE
widget, a graph laid out by processing order: a picture rail and a sound rail, a node per slot with
its own on-dot, the pinned head and the leading Geometry node in their places, and a tie between the
two halves of a paired device. No layout SHALL be stored. Selecting a node SHALL open that slot's
existing form below the graph. Adding SHALL go through the existing picker gallery at the place
asked; a drag SHALL reorder within its rail only; removing one half of a device SHALL remove both.
Every gesture SHALL be one undo step. No window SHALL be added for it.
Why: kkt-26.11.18b item 3 — "в морфере и впринципе в fx-chain управление дебильное … нужно что-то
типо компактного удобного графа"; R.63 — no new window; rule 17 — one widget.

#### Scenario: An event and its linked audio in one graph
- **PATH** double click a video event linked to its audio → the FX window shows one graph: Geometry, Color Curves and Glow on the picture rail, EQ on the sound rail → click `+` at the end of the picture rail → the picker gallery opens there → click VHS Tape → a node appears on the picture rail and "VHS Tape · sound" at the end of the sound rail, tied → the deck face opens below
- **WHEN** the person adds VHS Tape through the graph of a linked pair
- **THEN** the video chain ends with `vhs_tape`, the audio chain with `vhs_tape_sound`; the geometry dump names two rails, the tie and the selected node; one undo step removes both
- **TEST** t2994 (`src/apps/kt-app/src/fx_graph_tests.rs`)

#### Scenario: Reorder, refuse a cross-rail drop, remove a device
- **PATH** drag the "Glow" node right past "VHS Tape" → an insertion mark shows between the nodes → release → the order changes → drag "Glow" onto the sound rail → the refusal mark and the hint show → release → nothing changes → hover "VHS Tape" → click its `×` → both halves go → Ctrl+Z → both return
- **WHEN** the person reorders, drops across rails and removes a device
- **THEN** the picture chain is reordered in one step, the cross-rail drop changes nothing, the removal takes both halves with their curves and undo brings them back with their values
- **TEST** t2995 (`src/apps/kt-app/src/fx_graph_tests.rs`)

#### Scenario: The layers and the track stage are shown in their order
- **PATH** double click an event under two morpher layers on a track with a compressor keyed by the music track → the graph shows the event's chain, then the lower layer's folded purple group, then the upper's, then the track group with "← key: Music" → click the lower group → it unfolds in place, greyed → double click it → that morpher's window opens
- **WHEN** the FX window opens on an event reached by layers and a keyed track stage
- **THEN** the groups stand in the engine's order (event, layers bottom-up, track), their nodes are not editable there, and the double click opens the existing morpher window, no other
- **TEST** t2996 (`src/apps/kt-app/src/fx_graph_tests.rs`)

#### Scenario: A long chain and many partners stay compact
- **PATH** open the FX window on an event with 24 effects linked to three audio events → the picture rail scrolls sideways under the wheel while over it → each audio partner is a named rail under the first → no node name wraps
- **WHEN** a chain is longer than the window and the event has several partners
- **THEN** the graph's band keeps its height, scrolls horizontally, every node label is one elided line, and the window body keeps its vertical scroll (R.66)
- **TEST** t3001 (`src/apps/kt-app/src/fx_graph_tests.rs`)

#### Scenario: The graph costs nothing at rest
- **PATH** open the FX window and the morpher window on the heavy showcase project carrying VHS Tape, Datamosh and Camera ISP → move the mouse over a node and leave it still → only the hover tint changes → wait → no frame is asked
- **WHEN** the graph is open and nothing moves
- **THEN** the work counters show no layout of the graph on a hover frame and no frame at rest; the live idle run with both windows open stays at its rest load
- **TEST** t3003 (`src/apps/kt-app/src/idle_tests.rs`), t3006 live `idle` (`src/scripts/live/idle.py`)
