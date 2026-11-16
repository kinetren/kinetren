## ADDED Requirements

### Requirement: A loop's report names a moment's key by its complete composites
`kt-cli trace … loop` SHALL compare a moment's key on a later lap only with a COMPLETE composite of it on an
earlier lap; a moment complete for the first time SHALL read as never kept, not as a changed key.
Why: kkt-26.11.16 — his trace read 6–25 moments a lap as "the frame's key changed"; every complete composite
of a moment had one key, the earlier record was an incomplete composite.

#### Scenario: An incomplete lap is not a key change
- **WHEN** a moment is composed incomplete under one key on a lap and complete under another on the next
- **THEN** the report gives "never kept on an earlier lap", not a changed key
- **TEST** t2760 (`src/apps/kt-cli/src/trace_query/tests.rs`)
