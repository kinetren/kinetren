## ADDED Requirements

### Requirement: An export never reads a proxy
Every export body, the render window and the command-line export SHALL open sources by the media's path
and never by a proxy binding, whatever the preview's proxy switch says.
Why: invariant 11 — the export's bytes depend on the project only; Premiere's "exported my proxies".

#### Scenario: Export bytes are the same with proxies on
- **WHEN** a project is exported while the preview plays with proxies on, and again in a fresh process with no proxy file on disk
- **THEN** the two files' decoded frames and samples are byte-equal
- **TEST** t3445 (`src/libs/kt-engine/tests/proxy_swap.rs`)
