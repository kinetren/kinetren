## ADDED Requirements

### Requirement: A lower-class piece is admitted against the time in flight
On a bounded ring, a lower-class frame or compute job cut into pieces SHALL submit a piece and go on
encoding the next without waiting for the piece's completion; the next piece SHALL be admitted by the
ring against the predicted device time in flight (its first pass's measured time), waiting only while
the bound is full or a higher class waits. A ring with a display still never holds a piece of several
passes predicted over the bound in front of the display, and the cut frame's bytes equal the uncut
frame's.
Why: found-defects 392 — `Piece::cut` waited for the device to finish each piece before admitting the
next: ~45 % of a 720-line Datamosh walk in his trace was the arbiter's own waits.

#### Scenario: A cut goes on while the bound has room
- **WHEN** a compute ring with no display and a ring with a display each take a lower-class job cut into pieces whose predicted times fit the bound together, and pieces that do not
- **THEN** the pieces that fit are admitted without waiting for the earlier piece's completion; the ones that do not wait until the earlier completes; t2253 holds
- **TEST** t2356 (`src/libs/kt-gpu/src/arbiter.rs`), t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

### Requirement: The prefetch advances a run of stream frames as one walk
While playing, the prefetch thread SHALL advance a stream stage's frames ahead of the frame on screen,
up to the end of its window or of the loop region, as one walk to the last frame of that run, not as
one walk per frame; the walk reaches the frame on screen first. The player SHALL tell the stream memo its
view (the loop region's start while the playhead is inside it, else the frame on screen), from which on
a walk keeps every picture.
Why: found-defects 391 — one advance per frame made every position its own submission while playing.

#### Scenario: Frames ahead are one walk
- **WHEN** a mosh is advanced from the frame on screen through eight frames ahead
- **THEN** the history is computed by at most ⌈9 / batch⌉ + 1 jobs, and every one of the nine frames then needs nothing
- **TEST** t2360 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: A held stream frame is shown as catching up
A composite that shows a stream stage's held frame (playing, the stage behind the playhead) SHALL say
so with its result, and the preview SHALL draw a light "catching up" marker over the picture while the
frame on screen is held, never the Loading scrim; a complete frame of the moment asked removes it.
Why: kkt-26.11.10 J2 — a held stream frame looked like playback, the preview said nothing.

#### Scenario: The composite says it held
- **WHEN** a paced player plays over a mosh whose history is behind the playhead, then the history catches up
- **THEN** the composites that show a held frame say held, the ones after the catch-up do not
- **TEST** t2361 (`src/libs/kt-engine/tests/decode_storm.rs`)

## MODIFIED Requirements

### Requirement: The preview's stream memo is sized by the loop and the machine
The preview's stream memo SHALL start at 256 MB and, while a loop region is set over a stream stage,
SHALL want what that loop keeps — the stage's picture a position (half the stage's RGBA
frame), every position of the region plus an eighth — plus its checkpoint tier — two pictures every N
positions of the walk from the stage's chain anchor to the region's end — over the 256 MB working set,
and never above a quarter of the memory the process can still take, measured at run time when the loop,
the walk or the stage's size changes. The memory ledger SHALL grant that want after the kept composites'
(media-index), and the memo's bound SHALL be its base plus its grant; the pinned tier SHALL hold at most
half of the bound. The codec's per-frame working state (the clean encoder's) SHALL be evicted before the
pictures, past the newest few entries the next frame reads. With no loop, or with
the memory unknown, it SHALL be 256 MB; a smaller bound SHALL drop the least recently used entries at
once. The sizing SHALL be a trace record.

Why: found-defects 143 — at 1080 lines 256 MB held about 20 positions, so a longer loop recomputed its
codec history every lap; a constant fits one machine only. memory-ledger: the memo's quarter and the
present memo's growth were two budgets that did not see each other. found-defects 394: the walk from the
anchor to the loop was not counted, and was evicted and recomputed every lap.

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
