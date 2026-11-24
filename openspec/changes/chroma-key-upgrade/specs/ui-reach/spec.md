## ADDED Requirements

### Requirement: A key is analyzed and inspected where it is edited
The greenscreen's form SHALL offer "Analyze" beside the key colour in an event's, a morpher's and a track's chain. While
the Viewer is on a video node, its badge over the preview SHALL offer Composite, Matte and Status, and Screen for a key;
the view SHALL be a state of the Viewer alone: it SHALL NOT be a value of the project, SHALL NOT be rendered, and SHALL
end with the Viewer. An internal stage used for it SHALL NOT stand in any picker, and a file naming one SHALL be refused.
Why: F-MSK-004 trick 1 — a key cannot be tuned without seeing its matte; UI rules 13, 17.

#### Scenario: Key a shot by mouse
- **WHEN** a greenscreen is added to an event, "Analyze" is clicked, the key's node is Ctrl+Shift+clicked, "Status" is chosen in the badge, then Esc
- **THEN** the slot's rows are set in one undo step and the status line names the screen; the preview shows the tinted matte; after Esc the composite is back; the project's text never held the view and an export of the range is the composite
- **PATH** effects window → "+" → "Greenscreen" → the slot's form → click "Analyze" → the progress window, then the background is gone and the status line reads "Screen: green (31, 178, 64)…" → Ctrl+Shift+click the key's node → the badge "Viewer: Greenscreen" with four chips → click "Status" → a black and white matte with tinted dirt → Esc → the composite
- **TEST** t3563 (`src/apps/kt-app/src/node_editor_tests.rs`), live scenario `key`

#### Scenario: Analyze says why it cannot
- **WHEN** "Analyze" is clicked on an event with no saturated area, then on one whose tolerance has a curve, then an edit is made while it runs, then on a track with no event at the cursor
- **THEN** in turn: nothing changes and the status says there is no green or blue area; the colour is set and the status names the animated row that stayed; the result is not applied and the status says the project changed; the button does nothing and the status says why
- **PATH** the form → "Analyze" → a red status line "No saturated green or blue area in this picture" → another event, whose Tolerance has a key → "Analyze" → the colour changes, "Tolerance is animated and stays"
- **TEST** t3564 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The internal view stage is nowhere to be picked
- **WHEN** the effect picker, the library and the guides' list are read, and a project file holding a `matte_view` slot is opened
- **THEN** none of them names it; the file is refused naming the slot
- **PATH** effects window → "+" → search "matte" → no such effect → File → Open a file that holds the stage → an error naming the slot, the open project stays
- **TEST** t3566 (`src/apps/kt-app/src/reach_tests.rs`)
