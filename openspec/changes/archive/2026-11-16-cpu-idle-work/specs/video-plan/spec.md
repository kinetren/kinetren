## MODIFIED Requirements

### Requirement: A playing loop's unshown moments are composed below the display
While a loop region plays and the display has an executor, the player SHALL compose the loop's moments
the display's executor does not keep and whose frames are all decoded, on an executor other than the
display's and at the scheduler's `near` class (below the frame on screen on every ring), and SHALL keep
each complete composite on the display's executor as a slow-chain composite is kept. It SHALL ask no
decode and withdraw no request, work ahead of the playhead, skip for the lap a moment that comes out
incomplete (a stream stage short of its frame included) or that the display's memo does not keep, and
stop when the loop has no such moment left, the play or the loop stops, or the project changes. Nothing
SHALL run while nothing plays. `KT_LOOP_FILL=off` disables it.

How many moments it composes at once SHALL come from the machine at run time, never from a constant of
one machine: on a hardware device as many as the scheduler's pool leaves beside one worker for a person's
answer, at most the lower-class pieces a ring admits in front of the display; each on an engine of its
own on the same device (its own lock), no moment taken twice. On a hardware device a moment SHALL NOT wait
for the display to end a composite of its own (the ring puts the frame on screen first); on a software
rasterizer, where the fill and the display share the processor, the fill SHALL compose one moment at a
time and start none while the display composes. A moment that comes out incomplete SHALL narrow the fill
to one moment for the rest of the lap. `KT_LOOP_FILL_WIDTH=N` overrides the width.
Why: `found-defects.md` row 251 — on a display slower than the project's frames each lap showed a
different subset and only shown moments were kept: a covered loop composed "for the first time" lap
after lap. The width: the owner's `kkt-26.11.15` item 4 ("он тупо простаивает") — on his iGPU one fill
moment took ~170 ms of wall for ~20 ms of device work, one at a time, the processor at 13 % of one core
and the card a third busy.

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

#### Scenario: On a card the fill keeps several moments in flight
- **WHEN** a loop of 24 moments plays with a fill executor that reports a hardware device and whose every composite takes 40 ms of wall (a round trip), the display composing a frame of its own all the while, with the width the machine gives and with `KT_LOOP_FILL_WIDTH=1`
- **THEN** with the machine's width (at least two here) at least two fill composites run at the same time, each on its own engine, no moment is composed twice in a lap, the loop is all-kept in at most two thirds of the wall time of width 1, and the display's composite in progress did not stop the fill
- **TEST** t2860 (`src/libs/kt-engine/tests/loop_fill.rs`)

#### Scenario: The width comes from the machine and the device
- **WHEN** the width is asked for a software and a hardware device, for pools of 3 to 6 workers, and with `KT_LOOP_FILL_WIDTH` set
- **THEN** a software device gets 1, a hardware device the pool less one worker and at most the ring's lower-class pieces in flight, and the variable wins over both
- **TEST** t2861 (`src/libs/kt-engine/src/loop_fill.rs`)

#### Scenario: An incomplete moment narrows the fill for the lap
- **WHEN** a loop plays on a hardware fill executor at width three and one moment comes out incomplete
- **THEN** from then to the end of the lap no two fill composites run at once; a new lap widens it again
- **TEST** t2862 (`src/libs/kt-engine/tests/loop_fill.rs`)

#### Scenario: Measured on a software rasterizer
- **WHEN** `kt-cli preview <showcase> <from> <to> --loop S` runs on llvmpipe (release) with the fill and with `KT_LOOP_FILL=off`, and `kt-cli trace loop` reads each run
- **THEN** with the fill the loop becomes all-kept in fewer laps (measured 9 against 18) and the display shows no fewer frames; both runs are in the change's design record
- **TEST** bench `kt-cli preview --loop` + `kt-cli trace loop`
