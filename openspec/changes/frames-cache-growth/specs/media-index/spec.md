## ADDED Requirements

### Requirement: The preview's decoded-frame cache grows with its look-ahead and loop
The player's decoded-frame cache SHALL ask the memory ledger for what its look-ahead and its loop need
at the current preview quality and decode step: twice the bytes of the look-ahead window, or every frame
of the loop region plus an eighth and the window's room. Its bound SHALL be its base plus what the
ledger grants that want, read at use, so the look-ahead's half, the decoder's span room and the stream
pacing's quarter follow it as it moves. A grant taken back (an export's claim, a higher tenant) SHALL
evict at once down to the new bound; a lower want of its own SHALL evict in the asking call; holds and
pins SHALL stay stronger than any bound. A cache that never asks (an export's, a reel strip's, a
poster's) SHALL keep its base whatever the kind's grant, and a dropped cache SHALL give its want back
unless another has asked since. With the memory unknown the bound SHALL be the base. No bound SHALL
change a byte of a frame (invariant 11).
Why: found-defects 396 — memory-ledger left the frames cache with "no want today": the owner's trace
showed the frames cap at 576 MB for the whole run while ~21 GB of growth budget stood free, a loop
evicted on every lap (spikes of 537/s), and the preview quality changed nothing (kkt-26.11.10 J4).

#### Scenario: A loop past the base is held whole when the memory is there
- **WHEN** the player's prefetch passes run over a loop whose frames exceed the 512 MiB base, with memory to spare, then an export claims the growth and ends, then the preview quality drops
- **THEN** the bound rises past the loop's bytes on the first lap and the later laps evict nothing; the claim brings the bound to the base at once and its end gives it back; the lower quality shrinks the bound; with the memory unknown the same loop evicts on every lap
- **TEST** t2393 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)

#### Scenario: The cache's bound follows the ledger, and holds stay
- **WHEN** a cache asks for a loop's bound, a claim takes the growth while an entry is held, the claim ends, and the cache asks less
- **THEN** the bound is the base plus the grant; under the claim the cache evicts to the base at once and keeps the held entry; the grant comes back after it; the lower want evicts in the call
- **TEST** t2390 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Only the asker grows, and its want goes with it
- **WHEN** the player's cache asks while a job's cache holds frames, and when a cache that asked is dropped before and after another asked
- **THEN** the job's cache evicts against its base; the dropped cache's want leaves the ledger, but a newer want stands
- **TEST** t2391, t2392 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: The stream pacing orders by the bound it has now
- **WHEN** a renderer build is paced by the player's cache and the cache's bound grows after the build
- **THEN** the order takes a quarter of the grown bound
- **TEST** t2394 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)
