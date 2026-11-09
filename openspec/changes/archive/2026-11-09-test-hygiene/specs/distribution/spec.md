## ADDED Requirements

### Requirement: A test's clock is on the list or not in the test
Test code (a file of tests, or what follows a file's first `#[cfg(test)]`) SHALL NOT bound `.elapsed()`
by anything but `kt_time::TEST_WAIT`/`test_deadline()`, nor read the thread's processor time, outside
t1671's list of limits that are the check; each entry says why. A processor-time comparison SHALL
interleave the rounds of what it compares and take each one's cheapest.
Why: t1914 measured two scenes one after the other and read 8.5 ms against 4.2 ms at load ~60 on correct
code (found-defects 126); the scan of t1671 saw only `Duration::from_` bounds, and constants and
processor clocks passed it.

#### Scenario: The wider scan
- **WHEN** the sources of `src/apps`, `src/libs` and `src/plugins` are read
- **THEN** besides t1671's former patterns, no test code bounds `.elapsed()` by a constant or reads
  `thread_ns()` outside the list, and every entry of the list still exists
- **TEST** t1671 (`src/apps/kt-cli/tests/dist_guard.rs`); t1913, t1914 (`src/apps/kt-app/src/frame_cost_tests.rs`)
