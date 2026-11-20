## ADDED Requirements

### Requirement: The trace says what a restart cost and what the durable tiers restored
Over the first minute after a project opens the trace SHALL sum the history positions walked with their
device time and the covered audio bursts rendered again; every restore and write of a stream entry or a mix
run SHALL be a `render_cache` record of its own kind with its bytes. `kt-cli trace summary` SHALL print the
restart's cost, and `kt-cli trace cache` the entries and runs restored and written, with the history's raw
and stored bytes (its LZ4 ratio).
Why: durable-history-and-mix D1, D9 — the owner chose to build the tiers now (kkt-26.11.20); the trace shows
what they saved on his next restart and how much LZ4 shrinks his history (owner, 2026-11-09).

#### Scenario: The restart line
- **WHEN** a run opens a project and walks 300 history positions and renders 200 covered bursts again in its first minute
- **THEN** `trace summary` prints "after the start: 300 history positions walked" with their device time and "200 covered audio bursts rendered again"
- **TEST** t3336 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: `trace cache` counts the new tenants
- **WHEN** a run holds stream entries and mix runs restored and written
- **THEN** `trace cache` prints their counts and bytes per kind beside the composites, and the history's raw and stored bytes
- **TEST** t3337 (`src/apps/kt-cli/src/trace_query/tests.rs`), t3345 (`src/libs/kt-engine/src/render_cache.rs`)
