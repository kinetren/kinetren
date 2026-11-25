## ADDED Requirements

### Requirement: A generator that comes back has its source
A generator event that left what the player plays and came back with the same values SHALL get its source again: the undo
of a delete, a composition entered and left, an override set back.

Why: found-defects 921 — its address was remembered as raised, nobody decoded it, and the preview stood in Loading.

#### Scenario: A delete and its undo
- **WHEN** the project loses a generator event and gets it back, twice
- **THEN** a moment nobody asked before shows its picture each time
- **TEST** t3272 (`src/libs/kt-engine/tests/gen_event.rs`), live scenario `nestparam`
