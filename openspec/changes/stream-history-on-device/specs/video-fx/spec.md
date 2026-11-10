## ADDED Requirements

### Requirement: The datamosh walks its history on the device in batches
A datamosh history walk SHALL encode its positions in batches of several positions per submission to
the compute device: the source conversion, the clean encoder (search, residual, reconstruction), the
vector edits (swap, scale and rotation through a table computed on the CPU for every vector the search
can give, exact; macroblock loss by the same integer hash), the residual dropped for a lost block and
the broken decoder SHALL be device steps whose inputs stay on the device between positions. No
position SHALL wait for the device on its own: a walk waits once per batch. Only these pictures SHALL
come back to the CPU: the frame asked for; every position at or after the host's view (the loop region
when the playhead is in it, else the playhead; an export or a host without a view keeps every
position); every checkpoint position; the clean encoder's state where a later walk continues from it;
the clean picture of a frame with a region of interest. The pictures and keys SHALL be the bytes of the
per-position walk (invariant 11), for every row, gate and curve; a stopped or starved walk keeps every
position it computed before the stop.
Why: found-defects 391 — the walk ran position by position as two jobs, each a submit, a blocking wait
and a readback: 46 ms of round trip for 0.2 ms of device work, 7.5 positions/s while playing, the
device busy 12 % of a 720-line walk (his trace, kkt-26.11.10 J2/J3).

#### Scenario: The batched walk is the per-position walk
- **WHEN** a cut mosh, a stream with mosh/reset/bloom/freeze gates, per-frame scale, rotation, swap, loss and quantiser, a second input of two events and a held mosh past the refresh grid are rendered at every position by the batched walk and by the per-position reference walk, cold and from a memo holding every 7th picture
- **THEN** every picture and key is byte-equal
- **TEST** t2350 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: The vector edit on the device is the CPU's
- **WHEN** every vector of the search range (±64) goes through swap, scales of −16…16 and rotations of −180…180°, and loss of 0, 0.05, 0.5 and 1 with several seeds, on the device and on the CPU
- **THEN** the edited vectors, the lost blocks and the dropped residual are equal
- **TEST** t2352 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: A walk waits once per batch and reads back what is kept
- **WHEN** a 64-position cut mosh is walked cold with a view at position 48
- **THEN** the walk submits at most ⌈64 / batch⌉ + 1 jobs, reads back the pictures of positions 48–63 and of the checkpoints only, and its frame 63 is the reference's
- **TEST** t2351 (`src/plugins/fx-video-codec/tests/walk.rs`)

### Requirement: The datamosh keeps sparse checkpoints of its walk
A walk SHALL keep the broken decoder's reference and the clean encoder's state at every N-th position
(N fixed by the effect, counted from position 0) in the memo's pinned tier, outside the LRU. A later walk
of the same chain SHALL start at the latest checkpoint (or held picture) before its frame, so after an
eviction or a jump within an already walked chain it computes at most N positions before the first it
needs. When the pinned tier is over its bound, it SHALL drop checkpoints evenly (odd multiples of N
first, then of 2N), never the newest. The first visit of a chain still computes every position since its
anchor: the chain's length is the person's look decision (`gop`, `mosh`, `reset`).
Why: found-defects 394, 395 — the walk from the anchor was evicted and recomputed (1413 positions
computed, 618 distinct per lap in his trace); after a jump the coverage strip crept from the anchor.

#### Scenario: A re-walk is bounded by the checkpoint stride
- **WHEN** a 300-position mosh chain is walked, the memo's LRU entries are dropped (the pinned tier kept), and a frame at position 297 is asked
- **THEN** it needs at most N positions' source frames and computes at most N positions; the picture equals the cold walk's
- **TEST** t2353 (`src/plugins/fx-video-codec/tests/walk.rs`)

#### Scenario: The pinned tier thins evenly
- **WHEN** the pinned tier holding checkpoints at every N of 0…32N is bounded to half of what it holds
- **THEN** the checkpoints at even multiples of N remain and the newest stays
- **TEST** t2354 (`src/libs/kt-plugin-api/src/stream.rs`)
