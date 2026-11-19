## ADDED Requirements

### Requirement: A kt-app test process that dies natively leaves a raw report
A kt-app test process SHALL install the native-crash handler when it installs the plugin set, writing
the raw report to `<temp>/kt-test-crashes/<binary>-<pid>.raw` with the test's arguments in its header.

Why: found-defects 695, 783, 784 — reel scene tests died at exit ("double free or corruption", SIGSEGV)
under loaded full runs and left only their signal (poster-from-kept D3).

#### Scenario: An aborting test process leaves its report
- **WHEN** a kt-app test process installs the plugin set and aborts
- **THEN** a raw report of that binary names signal 6 and the test
- **TEST** t2979 (`src/apps/kt-app/src/plugins.rs`)
