# Spec Delta

## ADDED Requirements

### Requirement: Simultaneous openings of one sidecar all succeed
`MediaIndexDb::open` SHALL succeed for every one of several threads of one process opening the same
sidecar at the same moment, both when the file does not exist yet and when it exists and no connection
holds it. The setup of an opening (journal mode, schema) SHALL be taken in turn per sidecar path inside
the process; an opening of a sidecar that is already in WAL mode with the current schema SHALL NOT
write; the connection returned SHALL be the opening thread's own. A sidecar held past the busy timeout
by someone else stays a cache miss (t872).
Why: a project opening starts the index worker, the thumbnail worker, the reel-preview job and the
sources together, each opening the sidecar. Every opening wrote (`PRAGMA journal_mode=WAL`, the
schema); SQLite returned `SQLITE_BUSY` from the journal switch in 0.2 ms without the busy handler, and
on a new file a dozen openers outwaited the 5 s timeout. The reel preview then lost its disk tier and
rendered again on reopening — t640 red on a quiet machine, the owner's R.67 complaint by chance.

#### Scenario: A dozen threads open one sidecar at once
- **WHEN** twelve threads released by one barrier open the same new sidecar, twenty times; then the
  same twelve open it again after every connection has closed
- **THEN** every opening succeeds (before: one to three per run failed with "database is locked")
- **TEST** t1171 (`src/libs/kt-media/tests/index_db.rs`); its UI consequence — the second opening
  renders the reel zero times — t640 (`src/apps/kt-app/src/layout_tests.rs`)
