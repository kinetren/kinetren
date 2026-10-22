# Spec Delta

## ADDED Requirements

### Requirement: The owner's claims are indexed by the registry; the guard keeps it complete and honest
The owner's claims file SHALL be stored verbatim (`docs/ktprompts/<version>.txt`) and have a
registry `docs/ktprompts/<version>-index.md`, where EACH numbered item of the file has
a row with a state from the closed set: `done` (with the round and the test number),
`partial` (what exists, what is missing), `no` (a defect — with a capability; a feature — with a note in `docs/features`
or the mark "no note"), `question` (with the text of the question to the owner), `future`. A row
SHALL have a stage (a round number or "later"). The guard SHALL fail if a source item
has no row, a state is outside the set, a `done` has no test `tNNN` existing in the
sources, or a `question` has no text; the guard SHALL be part of `finish.sh`. The owner's answer to a
question SHALL be recorded verbatim in `docs/user-feedback.md`, and move the row from
`question` to a decision. An item that the OTIO exchange file cannot express SHALL go into
the registry section "what export loses".
Why: the owner asked "to make notes for agents and tests, so as not to return to the
same kinds of problems"; without a registry the next round would take from the file what is already done
(generator automation, freeze frame, moving away from OTIO) or would do the opposite of what was
decided (the format name, the import door). A rule in prose has already failed here twice
("do not make obvious labels" — checklist item 17 of R.7, and "Add morpher here").

#### Scenario: The registry is complete and honest
- **WHEN** the registry `v26.10` is checked; a row of one item is temporarily deleted; separately — a `done` row is given a non-existent test; separately — a `question` row is left without text
- **THEN** on the whole registry the guard is green and prints the count by state; each of the three corruptions makes it red with the item's address
- **TEST** t823 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/ktprompts-index.py`

#### Scenario: A closed item refers to a live test
- **WHEN** a registry row is in the state `done R.NN, tNNN`
- **THEN** the function `fn tNNN_` exists in the sources of `src/`, otherwise the guard is red — as with the registry of the migration of live scenarios
- **TEST** t823 (`src/apps/kt-app/src/reach_tests.rs`)
