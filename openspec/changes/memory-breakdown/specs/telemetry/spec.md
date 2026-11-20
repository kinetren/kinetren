## ADDED Requirements

### Requirement: The resident memory is split into the program's own sections
Besides the cache tenants, the project model, the undo history, peaks, thumbnails, reel previews, interface textures,
open decoders, the audio render's state and fonts SHALL report their bytes to the memory ledger as report-only
accounts that are never granted; the breakdown SHALL show each account and "not counted" as the resident memory minus
their sum, never negative; reporting SHALL change no cache's size.
Why: the owner, v26.10 Q3 — "распределить в ней информацию по нашему управлению памятью".

#### Scenario: The sections add up to the resident memory
- **WHEN** a project with 2 000 events, waveforms and thumbnails is opened, played for 10 s and paused; the breakdown is read twice
- **THEN** every listed account is positive where its holder holds data; accounts + not counted equal the resident reading of the same snapshot; the growing tenants' grants are equal with and without the report-only accounts
- **TEST** t3962 (`src/libs/kt-time/tests/mem_breakdown.rs`), t3963 (`src/apps/kt-app/src/diag_tests.rs`)
