## ADDED Requirements

### Requirement: A long frame says how long it waited for a processor
A UI frame of 100 ms or more SHALL be noted with how much of it the UI thread stood ready with no
processor (Linux: the run-queue wait of `/proc/thread-self/schedstat`), beside its stages' wall time. A
check of "no frame was blocked" SHALL judge the frame's wall time less that wait.
Why: at load 142 the live `ui_wait` saw a frame of 410 ms (centre 408 ms) whose work was 10 ms: the
machine's, not a lock's; nothing in the note told the two apart.

#### Scenario: The run-queue wait is read apart from a sleep
- **WHEN** the thread sleeps 50 ms; then spins with four times as many spinning threads as cores
- **THEN** the sleep adds under 50 ms of run-queue wait; the contention adds some
- **TEST** t2215 (`src/apps/kt-app/src/ui_wait_tests.rs`); live `ui_wait`

### Requirement: A recheck of kept moments asks the interface for no frame
While kept moments presented before an edit are rechecked against the new plan, the timeline SHALL ask
for no frame of its own; the preview's thread wakes the UI after each pass of the check.
Why: the timeline asked for the next frame on every frame while the check was pending, and on a
loaded machine the program ran ~150 frames a second "at rest" for seconds (live `edit_drag`,
`cached_range`, `idle`; found-defects 133, 170).

#### Scenario: A pending recheck
- **WHEN** kept moments are pending a recheck and two frames run
- **THEN** none of the repaint causes of the second frame comes from the timeline
- **TEST** t2217 (`src/apps/kt-app/src/ui_wait_tests.rs`); live `edit_drag`, `cached_range`, `idle`
