## MODIFIED Requirements

### Requirement: The trace says what the disk prerender did
Every restore of a moment from disk into the display's memo, every write, every eviction, every moment the
prerender measured (heavy or light), every moment it abandoned and every scan for the bars SHALL be a
`render_cache` record (`aux` the kind, `a` the tick, `b` bytes or device ns, `c` the durable key).
A written or evicted segment's record SHALL carry its moments and bytes, a written one the encoder's name;
a store that opens no encoder SHALL record why. `kt-cli trace cache` SHALL print per kind the count and the
bytes, the moments and segments written with their encoders, the moments restored per lap of a loop, the
prerender's heavy and light moments with their device time, why it stopped (input, Play, an export, the
cap, nothing left, no hardware encoder) and the reason the cache writes nothing.
Why: kkt-26.11.17 S5 — the work the program does by itself at rest must be readable from the owner's trace
(owner, 2026-11-09: diagnose from structured telemetry), or a busy machine at rest is a complaint nobody can
place.

#### Scenario: `trace cache` reads the tier's work
- **WHEN** a run holds two prerendered heavy moments, a light one, an abandon by input, a segment of two moments written by `h264_vaapi`, an eviction of four moments and three restores in one lap of a loop
- **THEN** `trace cache` prints the counts and bytes per kind, "written 2 moments in 1 segments (… encoder h264_vaapi)", "3 restored" for the lap, the heavy and light moments with their device times and "stopped: input 1"
- **TEST** t2987 (`src/apps/kt-cli/src/trace_query/tests.rs`)
