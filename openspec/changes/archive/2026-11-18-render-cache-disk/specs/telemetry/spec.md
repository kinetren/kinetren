## ADDED Requirements

### Requirement: The trace says what the disk prerender did
Every restore of a moment from disk into the display's memo, every write, every eviction, every moment the
prerender measured (heavy or light), every moment it abandoned and every scan for the bars SHALL be a
`render_cache` record (`aux` the kind, `a` the tick, `b` bytes or device ns, `c` the durable key).
`kt-cli trace cache` SHALL print per kind the count and the bytes, the moments restored per lap of a loop,
the prerender's heavy and light moments with their device time, and why it stopped (input, Play, an export,
the cap, nothing left).
Why: kkt-26.11.17 S5 — the work the program does by itself at rest must be readable from the owner's trace
(owner, 2026-11-09: diagnose from structured telemetry), or a busy machine at rest is a complaint nobody can
place.

#### Scenario: `trace cache` reads the tier's work
- **WHEN** a run holds two prerendered heavy moments (one written), a light one, an abandon by input, a write, an eviction and three restores in one lap of a loop
- **THEN** `trace cache` prints the counts and bytes per kind, "3 restored" for the lap, the heavy and light moments with their device times and "stopped: input 1"
- **TEST** t2987 (`src/apps/kt-cli/src/trace_query/tests.rs`)
