## MODIFIED Requirements

### Requirement: One memory ledger over every cache that holds pictures, mixes or device buffers
Every cache that holds decoded frames, kept composites, stream history, a kept final mix or device
buffers (the burst cache, the present memo, the stream memo, the front memo, the kept mix, the upload,
readback and transient pools, the compute pool) SHALL report the bytes it holds to one ledger of the process, under
its tenant kind. Each tenant SHALL keep its base — the working set it was proven with — whatever the
ledger says. What a tenant wants above its base SHALL be granted from ONE growth budget: half of what
the process could still take beyond its bases, measured at run time when a want changes (the memory
available now, a control group's limit included, plus the growth the tenants already hold). Grants
SHALL go in a fixed order of value — the kept final mix, then kept composites, then stream history, then
decoded frames — each
up to its want; taking back SHALL go the reverse way, and a tenant whose grant shrinks SHALL drop what
it no longer may hold at once. With the memory unknown nothing above the bases SHALL be granted. A hold
of an offline job SHALL stay stronger than any cap. No grant, cap or eviction SHALL change a byte of a
frame or a mix (invariant 11), and the ledger SHALL do nothing at rest: it acts only when a want, a
claim or a tenant's size changes.
Why: found-defects 310, M9 of `docs/engine-audit.md` — ten caches each picked their own constant, and
nobody saw the sum or took back a byte where it was worth least; the owner (kkt-26.11.08 H13): no
limit tuned for one machine. loop-keeps-its-lap (rows 542, 550): the kept mix was a fixed 64 MB and the
front memo stood outside the ledger; a minute of a loop's mix is 22 MB and saves the whole audio render,
so it is granted first.

#### Scenario: Grants follow the order of value within the budget
- **WHEN** the kept composites, the stream history and the decoded frames each want more than their bases, with plenty of memory, with memory for part of it, and with the memory unknown
- **THEN** with plenty each gets its want; with part, the kept composites are granted first, the stream history gets what is left and the decoded frames nothing; unknown — every cap is its base; the bases are never cut, and the sum of grants never passes half of the memory available plus the growth held
- **TEST** t2290 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: A higher want takes back the lowest grant first, and only a change acts
- **WHEN** the stream history holds its grant, then the kept composites' want rises past what is left, then the same wants are asked again
- **THEN** the stream history's cap drops and its owner is told at once, the kept composites' does not move down; asking the same wants again changes no cap, tells no owner and writes no record
- **TEST** t2291 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: The kept mix grows first
- **WHEN** the kept composites hold a grant, then the kept mix asks for its loop, then a claim is taken and dropped
- **THEN** the kept mix is granted before the kept composites, which give way and are told; the claim takes the mix's growth back to its base and its end gives it back
- **TEST** t2600 (`src/libs/kt-time/src/mem.rs`)

## ADDED Requirements

### Requirement: A loop longer than a cache keeps a stable part of it
Every cache a playing loop reads lap after lap SHALL, when the loop is longer than its bound, keep a
stable part of the loop from one lap to the next instead of evicting by recency alone. The decoded-frame
cache, told of the player's loop and its laps (where the stream memo is told), SHALL stamp each entry
with the lap of its last use (a read, a look-ahead refresh, its insertion); an entry put into a full pool
holding no entry unused for two laps SHALL enter a window, and the victim SHALL be, in order: past half
of the pool's bound, the window's oldest entry already read; the oldest kept entry (one no recent lap
used, if any); the window's oldest. An unread entry of the look-ahead SHALL NOT be dropped for the
window's sake. Holds and pins SHALL stay stronger. The same rule SHALL hold for the audio pool. The front
memo (the outputs of the slots in front of a stream stage) SHALL be a stream memo of its own, reported
to the memory ledger as stream history with a 64 MB base, told the preview's loop and laps with the
preview's stream memo. No rule SHALL change a byte of a frame (invariant 11).
Why: loop-keeps-its-lap (rows 551, 542) — an LRU over a cycle longer than itself evicts exactly what the
next lap reads first: past its grant the frames cache decoded every frame of every lap, and the front
memo, a fixed queue outside the ledger, recomputed every front of a loop.

#### Scenario: A loop twice the bound keeps half of it
- **WHEN** a loop of 20 frames is read lap after lap through a cache bounded to 10, plain and told of the loop
- **THEN** the plain cache hits nothing on any lap; told of the loop, every lap from the second hits at least 4, the same number each lap, within the bound and a window of half of it
- **TEST** t2604 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Stale entries go first, pins and holds stay
- **WHEN** entries outside the loop were unused for two laps, the loop fills the pool, then a held and a pinned entry stand while the loop runs on, then the loop is no longer kept
- **THEN** the stale entries go before any of the loop's, which are kept, not windowed; the held and pinned entries stay; no loop, no window
- **TEST** t2605 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: The player's look-ahead over a loop past the bound
- **WHEN** an 830 MB loop of three layers runs through the prefetch pass, the sizing and the composite's read with no memory to grow past the 512 MiB base, plain and told of the loop
- **THEN** the plain cache decodes every frame of every lap; told, every lap after the first decodes at most 650 of 900 frames, the same each lap, and no frame of the look-ahead is dropped unread
- **TEST** t2606 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)

#### Scenario: The front memo is in the ledger and keeps a part of a loop
- **WHEN** the preview's renderer tells its loop and laps, and a loop of front outputs twice the front memo's bound is made and read lap after lap
- **THEN** the ledger's stream history base holds the front memo's 64 MB, and every lap from the second reads at least a quarter of the loop from it
- **TEST** t2608 (`src/libs/kt-engine/tests/stream_memo.rs`)
