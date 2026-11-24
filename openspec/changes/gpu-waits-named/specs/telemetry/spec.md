## ADDED Requirements

### Requirement: The trace names every wait inside a frame's device part
A wait of half a millisecond or more inside a composite's device part SHALL be a record of the trace
(`gpu_wait`) that names what was waited for, its kind and the device's role: a lock or a turn somebody
else held (the GL gate, the engine, the pipelines' tables, the pass times, the display's hold), one call into
the device (a pipeline's creation, an upload, the uniforms, a submission, a poll, the wait for a submission
and its map), a file of the GPU layer read or written. `kt-cli trace frame` SHALL print those of the frame's
thread inside the executor's span, `during` SHALL sum them by name. At rest nothing is recorded.
Why: found-defects 835 — two frames stood 4.8 s and the trace held two empty spans.

#### Scenario: A wait past the threshold is a record that names it
- **WHEN** a thread takes a lock another holds, makes a device call and touches a file past the threshold, and takes a free lock
- **THEN** the first three are records with their name, kind and role, and the free lock leaves none
- **TEST** t3251 (`src/libs/kt-gpu/src/holds.rs`)

#### Scenario: The query says what a standing frame stood on
- **WHEN** a frame's executor stood 4.8 s on a lock while another thread wrote the file under it
- **THEN** `frame` prints the lock and its length and not the other thread's file; `during` lists both
- **TEST** t3253 (`src/apps/kt-cli/src/trace_query/tests.rs`)
