# Spec Delta

## MODIFIED Requirements

### Requirement: Multiple selection, marquee, moving a batch, double clicks
The selection SHALL be a set: Ctrl+click adds/removes and does not move
the cursor, a plain click selects one and places the cursor. The frame starts ONLY from
empty space (Ctrl+drag always; a plain drag — in the "Selection
tool" button mode); Ctrl on an event still disables snapping. Moving a batch moves only along
time, mutual shifts do not change, the stop is the first contact of any participant,
the commit is one batch from the ORIGINAL starts with the crossfade handles returned.
Operations that make no sense on a set require exactly one selected item and say so.
Double-click on an event — the FX window; a TRIPLE click on an event, Shift+double-click and the
menu item — the region from the event (one body: the span of the event's link group); double-click
on the ribbon of a range marker — the region along its bounds plus selection of its events.
A triple click SHALL leave nothing of its second click behind: the FX window that this gesture's
double click opened closes again, a reel that it expanded in place collapses again; on the level
line, whose double click resets the level, the third click does nothing more.
COPY (Ctrl+C and the menu item) SHALL take, in this order: (1) when the selection is exactly ONE
event (with its link partners, which travel with it anyway) — that event, whole, whether a region
exists or not; (2) otherwise, when a region exists — what lies in the region on all tracks,
cropped to it; (3) otherwise — the selected events with their link groups. The status line SHALL
name which of them was taken ("the selected event", "the region", "the selected events") and the
count; a region left in place by rule (1) is not cleared. The selection of morpher layers and
cameras does not take part in Copy (unchanged).
Why: "area selection is missing… ctrl… or as in Vegas — a toggle button";
the hint promised "double-click — FX" since R.18, but the gesture set a region.
ktprompts 26.10.18 (2026-10-27), the owner: "4. If I selected one event, copy-paste must take it,
not the region I selected. 5. A triple click on an event selects a region over its bounds." Before
this, the region always won: a loop region kept for listening made a click on one event and
Ctrl+C copy every event of every track inside the loop.

#### Scenario: Ctrl+click and frame
- **WHEN** Ctrl+click on events; Ctrl+drag from empty space; a plain drag from empty space without the button
- **THEN** the set grows and shrinks, the cursor stays; the frame catches ANY intersection and creates no region; without Ctrl a region is created as before
- **TEST** t202 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: "Selection tool" button
- **WHEN** the button is turned on by a click
- **THEN** a plain drag over empty space catches events with the frame; turning it off returns to the region
- **TEST** t203 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Three-step batch move
- **WHEN** two events are selected and one of them is dragged (press → 8 px → target)
- **THEN** both have moved by one shift, the unselected one stays in place, the durations are intact, the stop counts the block as a whole, undo returns it in one step
- **TEST** t204 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame becomes a named area
- **WHEN** after the frame, "save as area" is invoked
- **THEN** the area is created along the frame bounds in one history step, the name is made unique, the prefix on insertion is available from the same window
- **TEST** t248 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Operations over the whole selection
- **WHEN** Del, copy-paste, morpher targets on the set
- **THEN** they go over the whole selection in one history step; right-click on a member of the selection does not break the batch
- **TEST** t205 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Double clicks
- **WHEN** double-click on an event, Shift+double-click, double-click on the ribbon of a range marker (harness with a step of 0.02 s, a separate one for each gesture)
- **THEN** the FX window; a region from the event; a region from the marker with the events inside selected; with several overlapping, by flag/name
- **TEST** t206, t207 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: One selected event wins over the region
- **WHEN** a region covers three events on two tracks; one of them, which has an audio partner, is clicked; Ctrl+C; Ctrl+V at a free place
- **THEN** the clipboard holds that event and its partner, whole (not cropped to the region), and the paste puts exactly these two; the region is still in place with the loop state unchanged; the status line names "the selected event" and the count 2; on the code before the change the clipboard held all three events cropped to the region
- **PATH** drag on the ruler → the region over three events → click one event → it is highlighted, the region stays → Ctrl+C → the status line "Copied the selected event: 2" → move the playhead to empty time → Ctrl+V → the event and its audio appear there whole
- **TEST** t924 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Several selected events, nothing selected, no region
- **WHEN** a region exists and two events of different link groups are Ctrl+clicked; Ctrl+C; then Esc and the region is set again with nothing selected; Ctrl+C; then with no region two events are selected; Ctrl+C
- **THEN** the first copy is the region block (every crossing event, cropped) and the status line says "the region"; the second is the region block too; the third is the two events with their groups and the status line says "the selected events"
- **PATH** drag on the ruler → region → Ctrl+click two events → Ctrl+C → "Copied the region: N" → Esc → drag the region again → Ctrl+C → "Copied the region: N" → Esc twice → Ctrl+click two events → Ctrl+C → "Copied the selected events: 2"
- **TEST** t925 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Copy after a double-click on a range marker
- **WHEN** a range marker crosses three events and is double-clicked; Ctrl+C; separately a range marker that crosses exactly one event is double-clicked; Ctrl+C
- **THEN** with three events the region block is copied (cropped to the marker's range, the marker travels with it), as before the change; with one event that event is copied whole, because it is the one selected event, and the status line says "the selected event"
- **PATH** double-click the ribbon of a range marker over three events → region over it, three events highlighted → Ctrl+C → "Copied the region: 3" → double-click a range marker over one event → one event highlighted → Ctrl+C → "Copied the selected event: 1"
- **TEST** t926 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Triple click on an event sets the region over it
- **WHEN** an event with an audio partner and no FX window open is triple-clicked on its body (harness step 0.02 s between clicks); separately with the FX window of another event already open
- **THEN** the region equals the span of the event and its partner, the event is selected, no FX window of this event is open after the third click; the already open FX window of the other event stays open; one triple click adds no history step
- **PATH** triple-click the body of an event → the FX window flashes open on the second click and is gone after the third → the region band over exactly the event and its audio → press L → the loop plays exactly the event
- **TEST** t927 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Triple click on a reel and on the level line
- **WHEN** a reel instance is triple-clicked on its body; separately the level line of an audio event set to −6 dB is triple-clicked
- **THEN** on the reel the region covers the instance and the reel is NOT left expanded (the second click's expansion is undone); on the level line the gain is 0 dB (the double click's reset, one history step) and the region is unchanged
- **PATH** triple-click a reel instance → it expands on the second click and collapses back on the third → the region band over the instance → triple-click an audio event's level line → the line jumps to 0 dB, the region stays where it was
- **TEST** t928 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Three ways to a region over an event are one body and are promised
- **WHEN** for the same event the region is set by a triple click, by Shift+double-click and by the menu item "Region over this event"; the event's hover hint and the menu item's chord column are read
- **THEN** the three regions are equal; the hint of an event names the triple click next to "double click — FX" in all five catalogs; the menu item's chord column names the triple click; the hint does not promise a gesture the timeline does not perform (t83 family)
- **PATH** hover an event → the status line "… double click — FX, triple click — region …" → right-click it → "Region over this event   3×click" → click the item → the same region as the triple click gave
- **TEST** t929 (`src/apps/kt-app/src/reach_tests.rs`)
