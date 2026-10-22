# Spec Delta

## ADDED Requirements

### Requirement: Offline loudness and sync analysis — one job, dedup, negative cache
Media analysis (loudness, synchronization) SHALL run no more than one task at a time; the queue deduplicates requests; a failure is remembered and is NOT retried by itself (a retry only on a direct request). A hint, hover or window MUST NOT place a request; until it has been measured, this is said as such. `is_running` is held by a live task, not by an open window. A result for a stale revision or for vanished media MUST NOT be applied. The loudness of a source SHALL be computed during the pyramid pass and read from the DB on a repeated run.
Why: analysis reads whole files — three at once would take the disk away from playback; endlessly hammering an unavailable file; hovering the mouse over a bin would mean decoding a dozen files; a window left after finishing would keep the system warm; a second pass for LUFS — minutes on a one-hour source. Ducking left the list of tasks together with the snapshot (R.65): a live sidechain does not need offline analysis at all, it is computed in the audio render thread as an ordinary node.

#### Scenario: Gate, deduplication, negative cache, cancellation
- **WHEN** a duplicate and a second task are placed; a task fails; everything is cancelled
- **THEN** the duplicate is not in the queue, one is in progress; the refusal is remembered, is not retried by itself, and is retried on request — yes; cancellation clears the queue and the current task, and `is_running` goes out
- **TEST** t408, t409, t410 (`src/apps/kt-app/src/analysis.rs`)

#### Scenario: Loudness on the pyramid's pass and from the DB
- **WHEN** a file is indexed; the service is started a second time with the same DB
- **THEN** LUFS/dBTP are computed on the same pass; the second run returns the same number without decoding; synthetic material is not measured
- **TEST** t411 (`src/apps/kt-app/src/util.rs`)

#### Scenario: The hint shows and does not order
- **WHEN** the mouse moves over the cards of the bin
- **THEN** a known loudness is shown, an unknown one is labelled "not measured", and the number of analysis requests is zero
- **TEST** t412 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The ducking task no longer exists
- **WHEN** the list of background analyses and the kt-app sources are scanned for an on-request ducking task
- **THEN** a ducking task does not exist in the queue, the interface frame does not wait for it, and loudness and sync work as before
- **TEST** t774 (`src/apps/kt-app/src/analysis.rs`)

## REMOVED Requirements

### Requirement: Offline analysis — one job, dedup, negative cache, display does not order work
**Reason**: Ducking has been removed from the list of offline analyses (R.65: a live sidechain is computed in the audio render thread as an ordinary node, and it has no background task). The requirement is named by what is now actually computed offline — loudness and sync — and is therefore replaced rather than amended: a MODIFIED must carry the whole block in full, and there is no way to drop the scenario about the ducking task from it.

**Migration**: The rules of the queue, deduplication, the negative cache and "showing does not order" have not changed by a word and continue to apply to loudness and synchronization; tests t408–t412 are the same. Only the scenario about background ducking disappears together with the task itself.
