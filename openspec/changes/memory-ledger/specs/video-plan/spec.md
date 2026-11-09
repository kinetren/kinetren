## MODIFIED Requirements

### Requirement: Memoization of the composite and its key
A composed frame SHALL be memoized in the preview as a function of the whole plan, keyed by the plan key: on the readback path in the executor's readback memo, on the display path in the executor's present memo (`PresentMemo`), where presenting a held key runs only the display pass (`GpuStages.replayed`, the work row `composite_replays`). Only complete frames are kept. The present memo SHALL keep its 512 MB base and, while a loop region is set, SHALL want every composite of the region at the size it presents, plus an eighth; the memory ledger grants that want before any other tenant's growth (media-index). Without a loop or a grant it is bounded by its base, least recently shown first. The key SHALL include the hash of the chain with its
curves, the geometry of the clip, the resolved camera (it is selected per frame and is not visible to
a static hash), the masks of the frame, the decode step of the input, the parameters of the
transition kind, the step of the reel and the blend flag. Export MUST NOT memoize (each t is
composed once).
Why: any parameter that affects the bytes and is not in the key is a stale value, which shows up as "did not apply". preview-throughput D2 moved the preview to the display path, which had no memo: a covered range was composed again on every lap — 16 V-fps on the owner's project (kkt-26.11.07b, line 3). found-defects 310: the constant 512 MB held 582 composites at 640×360 and 60 at 1080p, and the owner's region is 665 frames, so its laps composed again every time (memory-ledger).

#### Scenario: The step is in the key
- **WHEN** one clip at one Quality is composed from input 1× and from input 2×
- **THEN** the two frames differ, and repeats return each its own bytes
- **TEST** t313 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Transition parameters in the GPU plan key
- **WHEN** each transition kind is executed at several phases, both axes of direction, and both states of the flags
- **THEN** the frames are within tolerance of their stored frames (volumetric ones — `GEOM`); a transition frame of another kind is not returned from the plan cache
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A presented composite is kept
- **WHEN** ten moments are presented twice, then one event's level changes and they are presented again
- **THEN** the second presents are kept composites with nothing composed and the same picture; after the edit exactly the moments of the edited event compose again
- **TEST** t1934 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: A granted loop is kept whole past the base
- **WHEN** the present memo's base holds ten composites, a loop of thirty moments is announced and granted, and the loop is presented three times
- **THEN** the second and third laps compose nothing and show every moment from the kept composites, the same pictures as the first; the memo holds the thirty past its base
- **TEST** t2292 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The player asks for the loop's composites
- **WHEN** a loop region is set on a playing player with a display executor, the preview size changes, and the region is cleared
- **THEN** the executor is told the region's moment count each time it changes, its want in the ledger is the region's composites at the presented size plus an eighth, and with no region the want is the base
- **TEST** t2296 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: The preview's stream memo is sized by the loop and the machine
The preview's stream memo SHALL start at 256 MB and, while a loop region is set over a stream stage,
SHALL want what that loop keeps — the stage's picture a position (half the stage's RGBA
frame), every position of the region plus an eighth, over the 256 MB working set — and never above a
quarter of the memory the process can still take, measured at run time when the loop or the stage's
size changes. The memory ledger SHALL grant that want after the kept composites' (media-index), and the
memo's bound SHALL be its base plus its grant. The codec's per-frame working state (the clean encoder's) SHALL be evicted before the
pictures, past the newest few entries the next frame reads. With no loop, or with
the memory unknown, it SHALL be 256 MB; a smaller bound SHALL drop the least recently used entries at
once. The sizing SHALL be a trace record.

Why: found-defects 143 — at 1080 lines 256 MB held about 20 positions, so a longer loop recomputed its
codec history every lap; a constant fits one machine only. memory-ledger: the memo's quarter and the
present memo's growth were two budgets that did not see each other.

#### Scenario: The bound follows the loop within the machine
- **WHEN** the bound is asked with no loop, with a loop needing less than the base, with one needing more on a machine with plenty and with little memory, and with the memory unknown
- **THEN** it is the base, the base plus the short loop's pictures, the base plus the loop's pictures, a quarter of the memory (not below the base), and the base; sizing the preview's memo sets its bound, drops what a smaller bound does not hold, and leaves a `stream_memo_cap` record
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: A loop the sized memo holds computes nothing on its second lap
- **WHEN** a mosh is advanced over a loop of positions twice, with a memo sized for that loop and with one that holds half of it
- **THEN** the second lap of the sized memo runs no encoder or decoder job and gives the same frames; the half memo recomputes
- **TEST** t2276 (`src/plugins/fx-video-codec/tests/packed.rs`)

#### Scenario: The kept composites come before the history
- **WHEN** the stream memo is sized for a loop while the kept composites want most of the growth budget, then the composites' want drops
- **THEN** the memo's bound is its base plus what the composites left, never above its own want; when the composites want less, the memo is granted its want again at its next sizing
- **TEST** t2297 (`src/libs/kt-engine/tests/stream_memo.rs`)

## ADDED Requirements

### Requirement: A loop longer than the present memo keeps a part of it
When the present memo cannot hold the loop it was told of, a new composite SHALL NOT be admitted over
a kept one that the current lap has shown or will still show (one shown within the loop's length of
presents); it SHALL be admitted over one older than that, least recent first. Without a loop the memo
SHALL stay least recently shown first.
Why: an LRU over a cycle longer than itself evicts exactly the moment the next lap reads first, so a
loop one frame too long for the memo kept nothing; with this rule it keeps what fits, lap after lap.

#### Scenario: A loop three times the memo keeps a third
- **WHEN** a memo holding ten composites is told of a loop of thirty moments with no grant, and the loop is presented three times
- **THEN** the second and third laps each show ten moments from the kept composites and compose twenty; without the loop told, the second lap shows none from them
- **TEST** t2293 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: A frame decoded again finds its kept composite
A moment whose decoded frames were evicted from the burst cache and decoded again with the same bytes
SHALL be presented from its kept composite: the re-decoded entries take back their serials, so the
plan key is the one the composite was kept under.
Why: found-defects 119 — a frame evicted and decoded again missed its kept composite and was composed
again; the serial return of structured-telemetry (t2152) fixed the cache, and this holds it end to end.

#### Scenario: Evicted, decoded again, replayed
- **WHEN** a moment is presented and kept, its frame is pushed out of a small burst cache by other frames, the same frame is decoded into it again, and the moment is presented again
- **THEN** the second present is a replay of the kept composite: nothing is composed and the picture is the same
- **TEST** t2294 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: A device's refusal bounds the present memo
On a device whose textures are not the process's own memory as the ledger counts it, the present memo
SHALL create the texture its growth needs under an out-of-memory check before keeping a composite past
its base. A refusal SHALL keep the composite unkept, SHALL bound the memo's growth at three quarters of
what it holds then (dropping the rest at once, so the device has room for its frames), SHALL be a trace
record, and SHALL NOT poison the device or lose the frame on screen.
Why: the memo's grant is measured in the process's memory; a card's own memory is not readable through
the graphics API, so it is learned at run time from the device's first refusal, never set as a
constant for one card.

#### Scenario: A refused growth is learned
- **WHEN** a memo holding twenty composites past its base meets a refusal at the next keep
- **THEN** that composite is shown but not kept, the memo holds fifteen past its base, later keeps never pass that bound, the frame on screen is complete and the device is not poisoned
- **TEST** t2295 (`src/libs/kt-gpu/tests/preview_display.rs`)
