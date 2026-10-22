# Spec Delta

## ADDED Requirements

### Requirement: The settings window and the geometry node are reachable from where the person acts
The project settings window SHALL be reachable from the project menu, by the address in the status line and
by a hotkey; the actions of the leading node (change the rule, reorder, add a second node)
SHALL be reachable from the event's chain editor; the window and the node SHALL have a full set of
verbs, checked by a machine against paired entities. A live scenario SHALL drop a vertical
file into a 16:9 project and assert the consequence: the preview with margins, the hint with the address, the window by address, changing the project's rule changes the picture.
Why: "Dead ends: you can create it, you cannot control it" repeated for seven rounds; the test must probe from the point where the person acts.

#### Scenario: Reachability and verbs
- **WHEN** the menus, the status line address, the hotkeys and the chain editor are scanned
- **THEN** the settings window and the "Geometry" node are reachable by each named path; the set of verbs is complete
- **TEST** t720 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A live scenario of a vertical source
- **WHEN** `src/scripts/live/conform.py` drops a vertical file into a 16:9 project, opens the window by address and changes the project rule
- **THEN** the screenshots show the margins, the hint, the window and a different picture after the rule change
- **TEST** `src/scripts/live/conform.py` (live scenario, `./scripts/live-check.sh`)
