## ADDED Requirements

### Requirement: The durable key's layout is a shared contract
The byte layout `render_cache::durable_hash` hashes (the plan's fields in `plan_hash` order without content
serials, `Leaves::digest`, the build salt) SHALL be one documented contract used by the disk prerender's
manifests and by the export's render index; a field added to `plan_hash` SHALL be added to it in the same
change, and a change of the layout SHALL move its `FORMAT` number, which makes every manifest and index of
the former number a miss.
Why: cached-final-render D9 — two stores keyed by one hash must not drift; a stale key would bring back a
frame of another picture from disk (render-cache-disk's rule).

#### Scenario: Every plan field reaches the durable key
- **WHEN** each field `plan_hash` reads is changed alone on a test plan
- **THEN** `durable_hash` changes for every one, and a manifest or index written with another `FORMAT` is a miss
- **TEST** t3335 (`src/libs/kt-engine/src/render_cache.rs`)
