## ADDED Requirements

### Requirement: The trace names a stream schedule's compile and the memo's sizing
The structured trace SHALL record each compile of a stream kernel's schedule (`stream_plan`: the
positions compiled and the positions held after it) and each sizing of the preview's stream memo
(`stream_memo_cap`: the bound, the loop's positions, the memory available then).

Why: the owner's run must say when a schedule was compiled and why the memo held what it held
(structured-telemetry; stream-layout-per-build).

#### Scenario: A compile is a record
- **WHEN** a stream layout with a cell is asked about a position, then about an earlier one, then about a later one
- **THEN** the trace holds two `stream_plan` records, the second starting where the first ended; the earlier position compiled nothing
- **TEST** t2277 (`src/plugins/fx-video-codec/tests/schedule.rs`)

#### Scenario: A sizing is a record
- **WHEN** the preview's memo is sized for a loop
- **THEN** one `stream_memo_cap` record carries the bound and the loop's positions
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)
