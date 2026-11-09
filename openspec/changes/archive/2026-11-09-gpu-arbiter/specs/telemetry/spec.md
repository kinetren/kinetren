## ADDED Requirements

### Requirement: Every ring decision is in the trace
Every admission to a ring SHALL be recorded with its class, the device time it predicted, the time it
waited and the ring; every cut piece and every stopped stream history SHALL be recorded too, with the
frame or job they belong to as the cause. `kt-cli trace gpu` SHALL report, per ring and per class,
submissions and compute jobs per second, admission waits and pieces, and the display's present
latency.
Why: `docs/engine-audit.md` §2b — "every job is counted per class"; row 134 asked whether the
look-ahead's Datamosh submissions fell, which only a count by class answers.

#### Scenario: The query reports the ring by class
- **WHEN** a window holds admissions of classes now and near on one ring, a cut frame of three pieces, a stopped history and three display presents
- **THEN** `gpu` lists per class the admissions, the waits and the submissions per second, the pieces of the cut frame, the stop, and the presents' latency
- **TEST** t2263 (`src/apps/kt-cli/src/trace_query/tests.rs`)
