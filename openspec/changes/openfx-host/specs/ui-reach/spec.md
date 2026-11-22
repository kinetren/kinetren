## ADDED Requirements

### Requirement: OpenFX plugins are picked like ours and marked foreign
The effect picker, the generator and transition lists SHALL show OFX plugins in their categories with a "foreign" mark
and a hint that their results may differ across machines; a host crash SHALL be one notice naming the plugin; the Library
SHALL list open OFX sets as installable items.
Why: the owner's O1; rule 16 — a difference is said, not hidden.

#### Scenario: Use an OFX blur
- **WHEN** the person opens an event's effects picker with openfx-misc installed and adds its blur
- **THEN** the card carries a "foreign" mark; the node is added and its rows are in the grid with curve buttons
- **PATH** double-click an event → effects window → picker → search "blur" → the openfx-misc card with a "foreign" mark → hover: the status bar says the result may differ on another machine → click → the node appears with its rows
- **TEST** t3977 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `ofx_plugin`
