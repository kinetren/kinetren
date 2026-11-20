## ADDED Requirements

### Requirement: Multicam is created, viewed and switched where the edit is
"Create multicam…" SHALL be in the selection's right-click menu and the Edit menu (with the sync method);
the angle viewer SHALL be a toggle in the preview head, disabled with its reason when the playhead is not
on a multicam instance; a click on a cell, a digit 1–9 and the menu item SHALL do the same switch, Alt
switching without a cut; the audio mode and "Expand multicam" SHALL be in the instance's right-click menu.
Why: UI rules 17 and 22; the owner: "не выкидывать фичи куда-попало".

#### Scenario: From three clips to a switched edit by mouse
- **WHEN** three camera clips are selected → right click → "Create multicam…" → "By sound" → OK; the "Angles" toggle is pressed; Play; cells 2 and 1 are clicked; Stop
- **THEN** one instance replaces the clips; the grid shows three angles beside the program picture, the active one outlined; two cuts appear at the clicked moments with angles 2 and 1; the sound stays on the first camera's microphone
- **PATH** select three clips → right click → "Create multicam…" → choose "By sound" → OK → one event with a multicam badge → click "Angles" in the preview head → a 2×2 grid beside the picture → Space → click cell 2 → a cut appears, the picture switches → click cell 1 → another cut → Space → the pass is one history step
- **TEST** t3513 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `multicam` (t3514)
