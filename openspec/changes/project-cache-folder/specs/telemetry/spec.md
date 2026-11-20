## ADDED Requirements

### Requirement: The trace names each cache of the project folder
Every cache of the project's cache folder SHALL report to the trace:
- its bytes at open and after each limit pass;
- its hits and misses;
- each eviction with its reason (the limit, another build, a broken entry).

The place of the folder and the reason for a fallback SHALL be a record of their own. `kt-cli trace cache`
SHALL print them per cache beside the prerender's records. `kt-cli cache <project> size|prune|clear`
SHALL show and act on a project's folder headless, and `--cache-dir` SHALL place it for a run.
Why: a "the cache does not hold" complaint is answered from the owner's trace (telemetry rule,
2026-11-09); project-cache-folder D8.

#### Scenario: The caches in the trace and headless
- **WHEN** a run opens a project, reads peaks and prerendered moments, and a limit pass evicts some, and then `kt-cli cache <project> size` runs
- **THEN** `trace cache` prints per cache the bytes, hits, misses and evictions with their reasons, and `kt-cli cache` prints the same sizes as the trace's last pass
- **TEST** t3841 (`src/apps/kt-cli/src/trace_query/tests.rs`)
