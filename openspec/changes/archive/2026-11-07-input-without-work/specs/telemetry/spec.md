## ADDED Requirements

### Requirement: The load watch never calls uncounted work rest
A second in which a thread other than the watch's own holds 10 % of a core or more while no work counter
moves SHALL NOT read "at rest": the report's line says "NO COUNTER MOVED" and Diagnostics says that no
counter moved. Model edits, project swaps and engine builds SHALL have work counters (`model_edits`,
`project_swaps`, `engine_builds`). The live scenario `idle` SHALL fail a pause that holds such a
thread.
Why: the owner's report read "at rest" at 196 % (its lines 482 and 512) while prefetch and the audio
render rebuilt the engine on every mouse move: the counters did not see that work, and the verdict
believed them. A tool made for proof must not hide what it was made to show.

#### Scenario: A busy thread with no counter
- **WHEN** a second has prefetch at 95 % and the audio render at 92 % of a core and every counter still, and another has only the watch's own tick
- **THEN** the first is not rest and reads "NO COUNTER MOVED", the second is rest
- **TEST** t1880 (`src/apps/kt-app/src/load_watch.rs`)
