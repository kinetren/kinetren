## ADDED Requirements

### Requirement: An export never reads the persistent cache
Every export body, the render window and the command-line export SHALL open sources by the media's path and never
by the persistent cache — no layer result, frame or source copy — whatever the preview's switch says.
Why: invariant 11 — the export's bytes depend on the project only; Premiere's "exported my proxies".

#### Scenario: Export bytes are the same with the persistent cache full
- **WHEN** a project is exported while the preview plays from cached stages, frames and copies, and again in a fresh process with an empty cache folder
- **THEN** the two files' decoded frames and samples are byte-equal
- **TEST** t3445 (`src/libs/kt-engine/tests/persistent_cache.rs`)
