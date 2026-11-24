## ADDED Requirements

### Requirement: An edit copies only the events it writes
The model SHALL share unchanged events between project versions, undo snapshots, the working view and the
player's snapshot: an edit of k events SHALL deep-copy O(k) events whatever the project's size, and the
project file, the clipboard text and every Debug-text key SHALL stay byte-equal to a model without sharing.
Why: found-defects 641 — the UI thread's handover was a whole-project clone per edit (10.9 ms at 41 357
events), and one `apply_batch` made four to five whole-project copies plus a full comparison
(shared-events D1, D2).

#### Scenario: One event's edit copies one event at any size
- **WHEN** one event's gain is set on projects of 1357 and 13 357 events, through `apply_batch` and through `apply_coalesced`
- **THEN** the counted event deep copies are the same small number on both sizes, and `doc_arc` after the edit copies none
- **TEST** t3367 (`src/libs/kt-model/tests/shared_events.rs`)

#### Scenario: Sharing is invisible to the file and the keys
- **WHEN** the heavy showcase is saved, its events' Debug text hashed and its clipboard block serialized, after a series of edits, undos and redos
- **THEN** the bytes and hashes equal those of the same project rebuilt from its text with no shared cells
- **TEST** t3368 (`src/libs/kt-model/tests/shared_events.rs`)

#### Scenario: A writer of every event copies only those it changes
- **WHEN** the lane order is changed so that three rows move on a project of 13 357 events
- **THEN** only the events on the moved rows are copied
- **TEST** t3369 (`src/libs/kt-model/tests/shared_events.rs`)

#### Scenario: A written cell never keeps a stale fingerprint
- **WHEN** every command family edits an event whose fingerprint was already asked
- **THEN** the cell's fingerprint after the edit equals the hash of the edited value's Debug text
- **TEST** t3370 (`src/libs/kt-model/tests/shared_events.rs`)

#### Scenario: No command copies events it does not write
- **WHEN** every command family is applied once to one event of a 13 357-event project (and the lane-order and track-renumbering commands to three rows)
- **THEN** each copies at most the events it writes; a command still writing through the container's `iter_mut` would copy every event and is named by the test
- **TEST** t3375 (`src/libs/kt-model/tests/shared_events.rs`)

#### Scenario: The shared vector is a vector
- **WHEN** a vector of values and the container made of it are printed (plain and pretty), serialized, read back, compared, cloned, written, filtered and drained
- **THEN** the text, the bytes and the values are equal; a clone copies no value, a write copies the one value it writes and only while another version holds it; the fingerprint equals the hash of the Debug text, is computed once per version, and a write clears it
- **TEST** t3376, t3377 (`src/libs/kt-model/src/shared.rs`)
