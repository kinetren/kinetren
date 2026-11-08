## ADDED Requirements

### Requirement: Validation, reconciliation and the round trip are linear in the project
`Project::validate` SHALL check a transition's ends through an index of its reel's events
(`Indexed`), `reconcile_reel` (every batch) SHALL find them by id, and the project file's write and
read SHALL stay linear, under the same counting guard as the builders.

#### Scenario: Many transitions
- **WHEN** 24, 96 and 384 pairs with a crossfade at every joint are validated and written and read back
- **THEN** walked elements and allocated bytes grow at most 8× and 32× (the former validation: ~4500× in events scanned)
- **TEST** t2062 (`src/libs/kt-engine/tests/build_cost.rs`)
