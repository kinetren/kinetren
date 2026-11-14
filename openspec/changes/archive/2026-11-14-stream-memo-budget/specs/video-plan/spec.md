## MODIFIED Requirements

### Requirement: The preview's stream memo is sized by the loop and the machine
The preview's stream memo SHALL start at 256 MB and, while a loop region is set over a stream stage,
SHALL want what that loop keeps — for every stream stage over the region, a reel's included, its own
positions there at its own picture (half the stage's RGBA frame at its size), plus an eighth, plus the
two pictures the next frame reads — plus each stage's checkpoint tier — two pictures every N positions
of the walk from the stage's chain anchor to its last position in the region — over the 256 MB working
set, and never above a quarter of the memory the process can still take, measured at run time when the
build, the loop or the quality changes (never at the moment on screen). The memory ledger SHALL grant that want after the kept composites'
(media-index), and the memo's bound SHALL be its base plus its grant; the pinned tier SHALL hold at most
half of the bound. The codec's per-frame working state (the clean encoder's) SHALL be evicted before the
pictures, past the newest few entries the next frame reads. With no loop, or with
the memory unknown, it SHALL be 256 MB; a smaller bound SHALL drop the least recently used entries at
once. The sizing SHALL be a trace record.

Why: found-defects 143 — at 1080 lines 256 MB held about 20 positions, so a longer loop recomputed its
codec history every lap; a constant fits one machine only. memory-ledger: the memo's quarter and the
present memo's growth were two budgets that did not see each other. found-defects 394: the walk from the
anchor to the loop was not counted, and was evicted and recomputed every lap. stream-memo-budget (rows 512, 518): the
want was every moment of the region at the size of the frame on screen and skipped reels — a short
Datamosh in a long loop over-asked History, which the ledger grants before Frames, and a stage of another
size re-sized the memo at each crossing.

#### Scenario: The bound follows the loop within the machine
- **WHEN** the bound is asked with no loop, with a loop needing less than the base, with one needing more on a machine with plenty and with little memory, and with the memory unknown
- **THEN** it is the base, the base plus the short loop's pictures, the base plus the loop's pictures, a quarter of the memory (not below the base), and the base; sizing the preview's memo sets its bound, drops what a smaller bound does not hold, and leaves a `stream_memo_cap` record
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: The want covers the walk's checkpoints
- **WHEN** the bound is asked for a loop of 40 positions whose chain anchor lies 400 positions before the region's end, on a machine with plenty of memory
- **THEN** it is the base plus the loop's pictures plus two pictures for every N of the 400 positions
- **TEST** t2355 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: A loop the sized memo holds computes nothing on its second lap
- **WHEN** a mosh is advanced over a loop of positions twice, with a memo sized for that loop and with one that holds half of it
- **THEN** the second lap of the sized memo runs no encoder or decoder job and gives the same frames; the half memo recomputes
- **TEST** t2276 (`src/plugins/fx-video-codec/tests/packed.rs`)

#### Scenario: The kept composites come before the history
- **WHEN** the stream memo is sized for a loop while the kept composites want most of the growth budget, then the composites' want drops
- **THEN** the memo's bound is its base plus what the composites left, never above its own want; when the composites want less, the memo is granted its want again at its next sizing
- **TEST** t2297 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: The want counts each stage's own positions, a reel's included
- **WHEN** a 6 s Datamosh sits at the root and a reel of it later, in a 60 s loop region otherwise plain, and the checkpoints of a walk are asked at the root and inside the reel
- **THEN** the want counts 2 × 180 positions at the stage's picture and both walks' checkpoint tiers, less than the region's 1801 moments at that size; a loop over the plain shot alone keeps nothing; the reel's walk names the same checkpoints as the root's
- **TEST** t2491 (`src/libs/kt-engine/tests/stream_memo.rs`), t2492 (`src/libs/kt-engine/tests/stream_memo.rs`)

## ADDED Requirements

### Requirement: A loop longer than the stream memo keeps a part of it
Told of a loop (`keep_loop`), the stream memo SHALL NOT evict an entry the current or the last lap used
for a new one while it holds anything older; a new entry put into a memo full of the loop's laps SHALL
enter a window of at most a quarter of the bound past the kept part, oldest out first, so the
composite still reads what the look-ahead just computed. The host SHALL mark a lap when its loop comes
round (`lap`); a new loop SHALL start a fresh count; without a loop the memo SHALL stay least recently
used first.
Why: stream-memo-budget (row 517) — an LRU over a cycle longer than itself evicts exactly the position
the next lap reads first, so a loop History could not hold recomputed every position every lap; the
present memo kept a part (memory-ledger D5), the stream memo did not.

#### Scenario: A loop three times the memo keeps a part
- **WHEN** a memo holding ten positions is run over a loop of thirty for several laps, each computed position read back at once, with the loop told and without; then over a loop it holds; then over a new loop after an old one
- **THEN** without the loop no lap finds anything; with it every lap after the first finds at least seven and the same number lap after lap; a held loop is found whole; the new loop keeps its own part
- **TEST** t2490 (`src/libs/kt-plugin-api/src/stream.rs`)
