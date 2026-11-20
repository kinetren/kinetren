## ADDED Requirements

### Requirement: An export never reads a disk copy
Every export body, the render window and the command-line export SHALL open sources by the media's path and never
by a disk-copy binding, whatever the preview's disk-copy switch says.
Why: invariant 11 — the export's bytes depend on the project only; Premiere's "exported my proxies".

#### Scenario: Export bytes are the same with disk copies on
- **WHEN** a project is exported while the preview plays from disk copies, and again in a fresh process with no copy file on disk
- **THEN** the two files' decoded frames and samples are byte-equal
- **TEST** t3445 (`src/libs/kt-engine/tests/disk_copies.rs`)
