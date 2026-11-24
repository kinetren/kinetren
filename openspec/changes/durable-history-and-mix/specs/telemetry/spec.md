## ADDED Requirements

### Requirement: The trace says what a restart cost and what the mix tier restored
Over the first minute after a project opens the trace SHALL sum the history positions walked from their cut with
their device time, the covered audio bursts rendered again and the bursts the mix tier restored; every restore and
write of a mix run SHALL be a `render_cache` record of its own kind with its bytes. `kt-cli trace summary` SHALL
print the restart's cost, and `kt-cli trace cache` the mix runs restored and written beside the composites.
Why: durable-history-and-mix D1 — the owner chose final frames only for the Datamosh history (kkt-26.11.22 D6), so
the walk from the cut after a restart is its price, shown on his projects; the mix tier's restores show what it
saved (owner, 2026-11-09: diagnose from the trace).

#### Scenario: The restart line
- **WHEN** a run opens a project and walks 300 history positions, renders 200 covered bursts again and restores 150 in its first minute
- **THEN** `trace summary` prints "after the start: 300 history positions walked" with their device time, "200 covered audio bursts rendered again" and "150 restored"
- **TEST** t3336 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: `trace cache` counts the mix tier
- **WHEN** a run holds mix runs restored and written
- **THEN** `trace cache` prints their counts and bytes beside the composites
- **TEST** t3337 (`src/apps/kt-cli/src/trace_query/tests.rs`)
