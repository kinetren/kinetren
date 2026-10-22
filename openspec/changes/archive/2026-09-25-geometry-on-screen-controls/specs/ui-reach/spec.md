# Spec Delta

## MODIFIED Requirements

### Requirement: The settings window and the geometry node are reachable from where the person acts
The project settings window SHALL be reachable from the project menu, from the address in the status bar, and by
a hotkey; the actions of the leading node (change the rule, reorder, add a second node) SHALL be reachable from the event's chain editor; the window and the node SHALL have a full set of verbs, checked by a machine against the paired entities. The live scenario SHALL drop a vertical file into a 16:9 project and assert the consequence: the preview with margins, the hint with an address, the window by the address, changing the project rule changes the picture.
A gesture over the FRAME SHALL be reachable and explained: the toggle for showing the controls lives in the preview itself, its state is visible, and a right click on the frame names it in words; the handle's hint says what it edits, in the status bar (rule 10 — no pop-ups). An event which has no frame (audio, a foreign placeholder) SHALL get a NAMED reason, not silence. The gesture SHALL work repeatedly (rule 14) and in a DETACHED preview window with the same body (rule 17).
Why: "Dead ends: you can create, you cannot control" was repeated for seven rounds; the test must probe from the point where the person acts. R.64: a control that one cannot learn about from the frame would repeat the Premiere complaint "I cannot summon the handles" (the reconnaissance F-UI-001).

#### Scenario: Reachability and verbs
- **WHEN** the menu, the address in the status bar, the hotkeys and the chain editor are scanned
- **THEN** the settings window and the "Geometry" node are reachable by each named path, the set of verbs is complete
- **TEST** t720 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A live scenario of a vertical source
- **WHEN** `src/scripts/live/conform.py` drops a vertical file into 16:9, opens the window by the address, and changes the project rule
- **THEN** the screenshots show the margins, the hint, the window, and a different picture after the rule change
- **TEST** `src/scripts/live/conform.py` (live scenario, `./scripts/live-check.sh`)

#### Scenario: Controls over the frame are reachable and explained
- **WHEN** the preview and its menu are scanned; a right click on the frame; hovering over a handle; an audio event is selected; the preview is detached into its own window
- **THEN** the toggle for showing the controls is found in the preview and in the right-click menu, its state is visible; the handle's hint is in the status bar; for an audio event the reason is named in words; in the detached window the gesture works with the same body
- **TEST** t766 (`src/apps/kt-app/src/reach_tests.rs`)
