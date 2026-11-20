## ADDED Requirements

### Requirement: A test waits for the code's last designed step, not for a flag before it
A test that asserts a state the code reaches after a designed step (a thread's pass after a close,
a lane releasing its state after it reports idle, a hold proven by a frame past a widened threshold)
SHALL wait for that step's own consequence, bounded by the test's guard, before it measures or
asserts; a fixed time or a flag the code sets before the step SHALL NOT start the measurement.
Why: t1644, t2380 and t1621 went red under load and green alone (found-defects 760, 781–783): each
measured from a moment that came before the step it asserted.

#### Scenario: The output thread's quiet is measured after its last pass
- **WHEN** the device closes at rest and the test measures that the output thread stands still
- **THEN** it first waits until the thread started and, after the close, until it made no pass for 100 ms; the quiet window then holds
- **TEST** t2380 (`src/apps/kt-app/src/audio_out.rs`)

#### Scenario: The drop's own record names its branch
- **WHEN** the effect-cell service is dropped with a cell running
- **THEN** the branch is taken from `shutdown::left_behind`; a left cell means the drop waited its bound; either way the worker's state goes within the guard
- **TEST** t1644 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A held Space plays once the hold is proven
- **WHEN** Space (and Ctrl+Space) is held from a pause
- **THEN** the test steps until it plays, never before the threshold, up to the guard
- **TEST** t1621 (`src/apps/kt-app/src/layout_tests.rs`)
