## ADDED Requirements

### Requirement: A paused preview whose sources do not answer runs no passes
While paused, a prefetch pass that left a stream's history pending, walked no position and saw no decoded frame arrive
SHALL park until a frame arrives or the state moves, with no clock. A frame that arrives SHALL wake it.
Why: found-defects 960 — 398 passes in 40 s on a pause, for a source that never answers; the far part of the look-ahead
had the rule, the near part did not (rest-is-said D6).

#### Scenario: A source that never answers, and one that answers late
- **WHEN** a paused preview stands on a Datamosh whose source holds every read, and the prefetch has parked with its requests out; a quiet window passes; then the source answers one frame at a time
- **THEN** no pass runs in the window; each frame runs a pass, and the moment comes out complete
- **TEST** t4123 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The preview's own wait is not lost
- **WHEN** the paused preview and the prefetch both wait for the next frame, and one frame arrives
- **THEN** both are called, each once
- **TEST** t1836 (`src/libs/kt-cache/src/lib.rs`), t4124 (`src/apps/kt-app/src/idle_tests.rs`)
