## ADDED Requirements

### Requirement: A live scenario reads the picture of its gesture
The geometry dump SHALL say whether the picture drawn is the one the program asks for now, and every reader of the
picture on the live stand SHALL wait for it after a gesture. A scenario SHALL read the screen's pixels only through such a reader.

Why: found-defects 845 — a scenario read the moment before its click.

#### Scenario: A seek's picture comes late
- **WHEN** the playhead moves while a composite is on its way
- **THEN** the dump says the picture is not current until that moment's whole composite is drawn, and the stand's readers wait for it
- **TEST** t3270 (`src/apps/kt-app/src/layout_tests.rs`), t3271 (`src/apps/kt-cli/tests/dist_guard.rs`), live scenarios `fade`, `gen_shapes`
