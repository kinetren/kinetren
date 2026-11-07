## MODIFIED Requirements

### Requirement: The burst cache is the only truth about coverage
The warmth bars SHALL be built only from `coverage()` of the cache itself — an interval set,
maintained on insert/evict, without parallel bookkeeping — and, for what plays without work, from
the two stores that hold the final products: the executor's present memo and the audio render
thread's final-mix store (capabilities `video-plan`, `audio-render`). Quality SHALL be part of the
key (the audio key has no quality); a quality with a different size MUST NOT satisfy the request.
The cache stores PRE-FX data; video post-FX is a separate kind, invisible to coverage, prefetch and pin,
but living in the video budget and removed by `invalidate_source`. The final mix is not in the burst
cache: the audio render thread keeps it.
Pin (the head of the loop region) excludes records from eviction, but not from coverage. The probe `contains`
MUST NOT touch the LRU and the epoch. Records evicted or overwritten before the first read
SHALL be counted by a separate counter (no-read drops).
Why: the old ms buckets of coverage lied in four ways (defect g); a probe-without-read that
refreshed the LRU evicted what had been honestly read; the "dead zeros" of the v1 performance panel.
"FX is cheaper than decode" did not hold for a simulated device on a nested reel (kkt-26.11.07b).

#### Scenario: Eviction shrinks coverage exactly, quality does not cross-satisfy
- **WHEN** records are evicted; another quality is requested; the cache does not mutate
- **THEN** coverage decreases exactly by the dropped records; another quality is a miss; the epoch changes only on mutation
- **TEST** t8 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: The head of the loop region is pinned
- **WHEN** a loop region plays with a warm cache
- **THEN** the wrap is sample-continuous, the head of the region is not evicted, there are no misses
- **TEST** t14 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: A probe does not touch the LRU and the epoch
- **WHEN** `contains` is called for records, and then an insert evicts
- **THEN** the epoch has not changed, the oldest by the REAL recency is evicted (the honestly read one survives)
- **TEST** t27 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Video coverage by ranges
- **WHEN** spans [pts, pts+step) are inserted and one is evicted
- **THEN** the strips merge into one; eviction removes exactly its own span; the interval-set operations are correct at the boundaries
- **TEST** t28 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: No-read drops are counted separately
- **WHEN** an unread record is evicted; a read record is evicted; an unread key is overwritten
- **THEN** the counter grows in the first and third cases and does not grow in the second
- **TEST** t37 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Source invalidation is not linear over the cache
- **WHEN** `invalidate_source` is called on every edit of a parameter
- **THEN** only the index of ITS OWN source is scanned
- **TEST** t497 (`src/libs/kt-cache/src/lib.rs`)
