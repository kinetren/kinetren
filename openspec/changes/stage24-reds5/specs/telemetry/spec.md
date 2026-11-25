## ADDED Requirements

### Requirement: A lap of a loop query is what was played
The loop and cache queries of the trace SHALL count a lap from Play, from a wrap or from a seek while playing. The
render thread's own seek at the place Play started SHALL cut no lap, and a seek of a paused transport SHALL start none.
`flush` SHALL say when the tail of the trace is not written, and the callers that end a run SHALL report it.

Why: found-defects 846, 918 — an empty lap read as "the first lap restored nothing", and which one it was depended on
two threads' order.

#### Scenario: Play, its own seek, a wrap and a pause
- **WHEN** a loop is opened, played, sought while playing, paused and sought again
- **THEN** the laps are the played stretches, whichever of the opening's records came first
- **TEST** t3269 (`src/apps/kt-cli/src/trace_query/tests.rs`)
