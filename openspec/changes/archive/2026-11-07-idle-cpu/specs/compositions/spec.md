## ADDED Requirements

### Requirement: A composition's previews settle
A release of a reel that an event of the current view shows SHALL NOT be evicted from the preview
service: `MAX_REELS_READY` bounds only what no event shows. A key that needs no job answers
`wants` with no copy of the document. Each set of overrides SHALL have its own row in the sidecar
(`reel_row`); an instance with no overrides keeps the reel's row.
Why: with more than six releases on the timeline the cap evicted the oldest, the frame asked for it,
its job evicted the next, and so on for as long as the program stood on a pause: 2 to 4 jobs and up to
60 device submits a second, 15 UI frames a second, 236 % of a core against 46 %, on a pause (E1). One
row per reel let the releases overwrite each other, so all but the last rendered on every opening
(E4). The request cloned the document for every instance on every frame until the reel was ready.

#### Scenario: More live releases than the cap
- **WHEN** ten releases are live and each has finished
- **THEN** all ten stay ready and none asks for a job; the cap still holds for keys nothing shows
- **TEST** t1830 (`src/apps/kt-app/src/idle_tests.rs`)

#### Scenario: Two releases in the sidecar
- **WHEN** two releases of one reel are written
- **THEN** each reads back, an instance with no overrides keeps the reel's row, and a row nothing uses is pruned
- **TEST** t1831 (`src/apps/kt-app/src/idle_tests.rs`)
