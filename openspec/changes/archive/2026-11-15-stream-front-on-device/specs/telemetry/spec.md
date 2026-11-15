## ADDED Requirements

### Requirement: A stale calibration places no device time
A compute job that finds another job calibrating SHALL go on without waiting, and SHALL NOT place its
device-time records on a calibration older than its age: such a record says it has no position
(`aux` = u32::MAX − 1) and is counted (`compute::stale_records`). Reading the calibration SHALL NOT
wait for one in progress. The next job calibrates again.
Why: found-defects 562 — gpu-leftovers D1's `try_lock` let such a job record on the older
calibration, which places its device times on a drifted clock.

#### Scenario: A job past the calibration's age while another calibrates
- **WHEN** the calibration is 11 s old, another job holds it, and a job runs with the trace recording
- **THEN** the job's `gpu_start` records carry no position, the stale count grows, and the next job places its record
- **TEST** t2623 (`src/libs/kt-gpu/tests/compute_start.rs`)
