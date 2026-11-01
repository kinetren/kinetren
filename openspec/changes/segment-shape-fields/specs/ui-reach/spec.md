## ADDED Requirements

### Requirement: Selecting segments is reachable by mouse and keyboard and is said
The segment selection of the automation graph SHALL be said where the person acts: the graph's
hover hint over a segment names the gestures (Ctrl+click, Shift+click, a drag over empty space), the
selection row names what it edits (the bounds of one segment or the count), and the segment menu
header names how many segments it touches. The selection row's controls SHALL be reachable by Tab in
the window's order, a typed value SHALL apply on Enter, and the row SHALL carry a button that clears
the selection. The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01 item 10 — the owner did not find where to set one segment's parameters; a selection
nobody can discover is the same "nowhere to write them" (rule 16: a control says what it does).

#### Scenario: The keyboard path through the selection row, and its clear button
- **WHEN** one zigzag segment is selected; the focus is moved by Tab from the "all" row's last control until it lands on the selection row's periods field; 3 is typed and Enter pressed; then the row's clear button is pressed
- **THEN** the periods field is reached by Tab alone, the segment has periods 3 after Enter, the other segments are untouched; after the clear button the row is gone and nothing in the curve changed
- **PATH** right click on a segment, Esc → Tab, Tab… → the periods field of "Selected segment" gets the focus frame → type 3, Enter → the segment's zigzag thins → click × at the end of the row → the row disappears, the tint goes off
- **TEST** t1121 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The gestures are said in the hint, the row and the menu, in every language
- **WHEN** the pointer hovers a segment of the active curve; a selection of two segments exists; the segment menu is opened on one of them; the catalogs are read
- **THEN** the hover hint names Ctrl+click, Shift+click and the sweep; the row caption names the count; the menu header names the count; every new key exists in en, ru, es, zh-Hans and ar
- **PATH** hover a segment → the status line explains how to select several → Ctrl+click a second segment → the row says "Selected segments: 2" → right click on one → the menu header says 2 segments
- **TEST** t1122 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/segfields.py`
