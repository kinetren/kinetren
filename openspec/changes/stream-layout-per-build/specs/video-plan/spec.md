## ADDED Requirements

### Requirement: The preview's stream memo is sized by the loop and the machine
The preview's stream memo SHALL start at 256 MB and, while a loop region is set over a stream stage,
SHALL be bounded by what that loop needs — two frames of the stage's size a position, every position
of the region plus an eighth — never below 256 MB and never above a quarter of the memory the process
can still take, measured at run time when the loop or the stage's size changes. With no loop, or with
the memory unknown, it SHALL be 256 MB; a smaller bound SHALL drop the least recently used entries at
once. The sizing SHALL be a trace record.

Why: found-defects 143 — at 1080 lines 256 MB held about 20 positions, so a longer loop recomputed its
codec history every lap; a constant fits one machine only.

#### Scenario: The bound follows the loop within the machine
- **WHEN** the bound is asked with no loop, with a loop needing less than the base, with one needing more on a machine with plenty and with little memory, and with the memory unknown
- **THEN** it is the base, the base, the loop's need, a quarter of the memory (not below the base), and the base; sizing the preview's memo sets its bound, drops what a smaller bound does not hold, and leaves a `stream_memo_cap` record
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: A loop the sized memo holds computes nothing on its second lap
- **WHEN** a mosh is advanced over a loop of positions twice, with a memo sized for that loop and with one that holds half of it
- **THEN** the second lap of the sized memo runs no encoder or decoder job and gives the same frames; the half memo recomputes
- **TEST** t2276 (`src/plugins/fx-video-codec/tests/packed.rs`)
