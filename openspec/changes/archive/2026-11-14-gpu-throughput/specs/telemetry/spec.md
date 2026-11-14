## ADDED Requirements

### Requirement: Every piece of device work says when it ran and who ran it
Every piece a compute job is cut into SHALL leave its own `gpu_start` record — its submit, its first
dispatch's start and its last's end on the device, on the trace's clock — from a timestamp set of its own
job: concurrent jobs SHALL NOT share a query set (a pool made up front; a job finding none free runs
untimed). Every piece the device runs and every CPU span that does a device's work SHALL leave a `gpu_exec`
record naming its executor. The model's edit record SHALL carry the model's revision. `kt-cli trace gpu`
SHALL report a walk's admission share (admission waits over the job's wall) and the device's idle gaps
between consecutive pieces.
Why: found-defects 486 — `GpuStart.a` was the job's last submit, `b` came from a query set every job shared
(a prefetch job's start equalled the compose job's; up to 27 ms of error), `Edit.a` was always 0, and no
record said which executor ran a piece.

#### Scenario: Concurrent jobs keep their own starts
- **WHEN** two compute jobs run at once on two threads, one cut into pieces
- **THEN** each piece has a `gpu_start` whose submit is its own and whose device start is at or after it, no two jobs' records share a start, and each piece has a `gpu_exec` naming the device
- **TEST** t2437 (`src/libs/kt-gpu/tests/compute_start.rs`)

#### Scenario: The report tells admission from device
- **WHEN** `trace gpu` reads a trace of a cut job whose pieces waited for admission and ran with gaps
- **THEN** it prints the walk's admission share and the device's idle gaps between pieces per class
- **TEST** t2438 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: An edit names its revision
- **WHEN** the player takes a project at a model revision
- **THEN** the `edit` record carries that revision in `a` and the player's version in `b`
- **TEST** t2439 (`src/libs/kt-engine/tests/player_trace.rs`)
