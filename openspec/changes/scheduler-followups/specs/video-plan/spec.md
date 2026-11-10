## ADDED Requirements

### Requirement: A playing loop's unshown moments are composed below the display
While a loop region plays and the display has an executor, the player SHALL compose, one at a time,
the loop's moments the display's executor does not keep and whose frames are all decoded, on an
executor other than the display's and at the scheduler's `near` class (below the frame on screen on
every ring), and SHALL keep each complete composite on the display's executor as a slow-chain
composite is kept. It SHALL ask no decode and withdraw no request, work ahead of the playhead, skip
for the lap a moment that comes out incomplete or held or that the display's memo does not keep,
and stop when the loop has no such moment left, the play or the loop stops, or the project changes.
Nothing SHALL run while nothing plays. `KT_LOOP_FILL=off` disables it.
Why: `found-defects.md` row 251 — on a display slower than the project's frames each lap showed a
different subset and only shown moments were kept: a covered loop composed "for the first time" lap
after lap.

#### Scenario: A loop shown in part becomes all-kept
- **WHEN** a loop of 24 moments plays with its frames decoded and a display that presents only every fourth moment it reaches, for three laps, with the fill and with `KT_LOOP_FILL=off`
- **THEN** with the fill every moment of the loop is kept by the display's executor by the third lap and the display's own composites of the last lap are all replays; without it a part stays unkept
- **TEST** t2334 (`src/libs/kt-engine/tests/loop_fill.rs`)

#### Scenario: The fill asks no decode and rests with the player
- **WHEN** a loop plays with half its frames not decoded, then the play stops
- **THEN** the fill composes only moments whose frames are decoded, the workers received no request from it, and after the stop it runs no job
- **TEST** t2335 (`src/libs/kt-engine/tests/loop_fill.rs`)

#### Scenario: The fill stops where the memo stops keeping
- **WHEN** a loop plays whose moments the display's memo refuses past the first eight
- **THEN** the fill keeps eight, skips the rest for the lap, and composes nothing more until the next lap
- **TEST** t2336 (`src/libs/kt-engine/tests/loop_fill.rs`)

#### Scenario: Measured on a software rasterizer
- **WHEN** `kt-cli preview <showcase> <from> <to> --loop S` runs on llvmpipe (release) with the fill and with `KT_LOOP_FILL=off`, and `kt-cli trace loop` reads each run
- **THEN** with the fill the laps after the second compose no moment the display asks for (all kept), without it each lap composes again; both numbers are in the change's design record
- **TEST** bench `kt-cli preview --loop` + `kt-cli trace loop`

### Requirement: The slow-chain frame never waits behind side work on its engine
Effect cells, reel strips and project posters SHALL compose on an engine of their own on the
offline device that shares its compiled pipelines but not the frame's state or lock; the slow-chain
route of the preview SHALL have the offline engine to itself. The offline device's ring SHALL bound
its flight once the slow route serves the preview from it: a lower-class frame submitted there is
cut at pass boundaries when the slow route's frame waits, so that frame waits for at most one piece.
Why: `found-defects.md` row 290 — the offline engine's lock was held for a whole frame, so the
slow-route frame waited behind an effect cell or a poster on the mutex, not on the ring that bounds
it; on a software device that ring did not bound either.

#### Scenario: A side frame holds the slow route for one piece
- **WHEN** a heavy chain composes on the side engine of the offline device (class background) and the slow-route engine asks a light frame at class now meanwhile, with the side engine's frame held mid-way
- **THEN** the light frame composes without waiting for the side engine's lock, its wait on the ring stays within one measured piece of the heavy chain, and both frames' bytes equal their solo runs
- **TEST** t2337 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The side engine compiles nothing
- **WHEN** the side engine is made after the offline engine
- **THEN** it shares the offline engine's compiled pipelines (no pipeline is created) and its own arena and pools
- **TEST** t2338 (`src/libs/kt-gpu/tests/arbiter.rs`)
