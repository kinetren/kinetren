## ADDED Requirements

### Requirement: A panic in a pump does not end its queue
A service's pump that panics SHALL be run again for the items behind the one that panicked, at most a bounded
number of runs in a row without a kick.

#### Scenario: A broken item among three
- **WHEN** the second of three items panics, and when a pump panics at every run
- **THEN** the third item is done, and the other pump stops after the bound
- **TEST** t3265 (`src/libs/kt-time/src/sched/tests.rs`)
