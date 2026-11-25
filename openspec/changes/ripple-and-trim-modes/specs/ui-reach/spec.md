## ADDED Requirements

### Requirement: Every ripple and trim command has its menu, toolbar and key
Post-edit ripple (each scope), the auto-ripple mode, close gaps, and the roll, slip and slide modifiers
SHALL be reachable from the Edit menu and the event's right-click menu, the mode from the toolbar, and
each SHALL be a row of the controls table with a key in every style that has one (ours: F and Ctrl+Shift+F for
the post-edit ripple), the menu printing the key from the table.
Why: UI rule 17 — one command, one behaviour from every entry; t1715 — no literal key strings.

#### Scenario: The commands are found in every entry and layout
- **WHEN** each layout of the controls table is selected and the Edit menu, the event's right-click menu and the toolbar are scanned
- **THEN** every ripple command is present with the key its layout gives, no two commands share a key in one layout, the menu label comes from the table
- **PATH** click "Edit" → "Ripple after the edit" with three scopes and their keys; right click on an event → the same items; the toolbar → the auto-ripple button with its hint key
- **TEST** t3431 (`src/apps/kt-app/src/reach_tests.rs`)
