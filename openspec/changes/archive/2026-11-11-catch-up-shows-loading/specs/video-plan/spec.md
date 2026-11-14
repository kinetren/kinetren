## MODIFIED Requirements

### Requirement: A live preview paces a stream stage's history
In the live preview (`StreamPacing`, set by the player on its renderers; `None` for an export and every
other path, whose bytes do not move) the order of a stream layer SHALL be the earliest positions its
history lacks that fit a quarter of the video cache, at least one, in the order the stage reads them. The
prefetch thread SHALL advance the stream stages over what the decoder brought — at the frame on screen and,
while playing, ahead of it (wrapping in the loop region) by the frames the display reaches before its next
pass, twice over, bounded by a quarter of the stage's memo — and a pass that leaves a stage pending SHALL
run again even on a pause. A composite SHALL run a stage only when its frame lacks at most two positions;
otherwise, playing or paused alike, it reports the frame incomplete and says how many positions the
history still lacks (the most over the frame's stream stages, reels and track stages included). An
offline path with its own sources and a small cache (the reel strip, the project poster) SHALL pre-roll a
stream stage as the export does, in batches of half its cache.
Why: ordering the whole history (up to 600 frames of 8 MB at 1080 lines against a 512 MB cache) let the
decoder run through it while the stage read only at composites; the cache dropped the history's front, the
frame never came, and the owner saw a loop at V-fps 0. With the history cached, the composite computed it
on the UI thread: 9 s for one composite on his relinked project (decode-request-storm). The playing
preview once showed the stage's latest held picture instead; the owner decided against it (kkt-26.11.10b,
found-defects 403: "ну пусть loading показывает").

#### Scenario: The paced order is the earliest of the history
- **WHEN** a frame 3 s into a cut mosh (a ~100-frame history) is ordered with budgets of 1, 7 and 30 frames, and of one byte
- **THEN** each order equals the first 1, 7, 30 and 1 entries of the unpaced order
- **TEST** t2042 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The advance computes the history, not the composite
- **WHEN** every frame of that history is cached and a paced renderer composes the frame, then runs `stream_advance`, then composes again
- **THEN** the first composite is incomplete and the memo untouched; the advance reports nothing pending; the second composite is complete and byte-identical to the unpaced renderer's; the next frame, one step on, is the composite's own
- **TEST** t2043 (`src/libs/kt-engine/tests/decode_storm.rs`)

## REMOVED Requirements

### Requirement: A held stream frame is shown as catching up
**Reason**: the owner decided (kkt-26.11.10b, found-defects 403, K5): a held picture is another
position's, and the stages after the stream stage ran over it at the moment asked — "либо есть кадр либо
нет". No composite shows a held frame any more.
**Migration**: "No stage runs over a stand-in input" (this capability) and "Loading names a stream
catch-up where it is written" (`ui-timeline`); t2361 is replaced by t2410.

## ADDED Requirements

### Requirement: No stage runs over a stand-in input
A composite SHALL NOT give any stage of the plan an input that is not that stage's own position — a
stream stage's picture of another position, a sub-composite of a reel or of a track stage that lacks a
layer, a neighbour of a frame pair, a decoded frame of another time — and run the stages after it (the
event's later effects such as VHS Tape or Camera ISP, the track's stages, the reel's stages) over it at
the moment asked. A layer whose own input is not there SHALL leave the frame incomplete; an incomplete
composite SHALL NOT be presented over the picture on screen and SHALL NOT be kept as the moment's
composite; the preview keeps its last full picture under its Loading look. The only incomplete
composite ever presented is the first picture of a preview that has shown none (the owner's rule from
round 14 items 14–15: a picture under the Loading scrim rather than an empty one). A kept composite of
the exact moment SHALL still show at once, without the history. An export SHALL compose every frame from
its own positions, never short.
Why: kkt-26.11.10b K5 — "при кэтчинге - у меня vhs tape будто бы накладывается в этот момент
актуальный … либо есть кадр либо нет, зачем накладывать эффект если предыдущий не готов?": the
playing preview ran the event's VHS Tape at the moment asked over a Datamosh picture of a position
the history had reached.

#### Scenario: A playing frame whose history is behind is Loading, not a held picture
- **WHEN** a paced renderer composes a frame of a cut mosh whose history the prefetch has walked to ten positions behind it, then the history catches up
- **THEN** the first composite is incomplete, the mosh layer has no picture, the renderer reports at least eight positions short, and the executor presents nothing when asked not to show an incomplete frame; after the catch-up the composite is complete, reports nothing short and is byte-identical to the unpaced renderer's
- **TEST** t2410 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: VHS Tape is never laid over another position's mosh
- **WHEN** the mosh event carries VHS Tape after Datamosh and its track carries VHS Tape as a track stage, and the frame is composed while the history is behind and again after it caught up
- **THEN** the short composite runs no chain of the mosh layer and is incomplete with nothing presented; the caught-up composite equals the unpaced renderer's bytes
- **TEST** t2411 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: An export composes every frame from its own positions
- **WHEN** a range of the mosh event with VHS Tape after it is exported through the serial export and through the pipeline
- **THEN** no frame is short, every frame's stream key is the stage's key of its own position, and the frames equal the unpaced renderer's at each moment
- **TEST** t2412 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A reel that holds a mosh reports its catch-up to the root
- **WHEN** the mosh project is placed as a reel and the root frame is composed through a paced renderer with the inner history behind
- **THEN** the root composite is incomplete and reports the inner positions short; after the inner history catches up the root composite is complete and reports nothing short
- **TEST** t2413 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The player answers a short frame with its catch-up and a kept moment at once
- **WHEN** a playing player is asked a frame of the mosh whose history is behind, then the same moment after its composite was kept and the memo dropped
- **THEN** the first answer is incomplete with a catch-up count above zero; the kept moment's answer is complete with a catch-up of zero, without the history
- **TEST** t2414 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A frame pair lacking a side is incomplete
- **WHEN** a slowed event in the frame mode "blend" is composed at a moment between two source frames with both frames cached, then with the neighbour missing, then with the base frame missing
- **THEN** the first is complete and blended; the other two are incomplete and their layer has no picture (neither side stands in at full weight)
- **TEST** t2418 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: An offline frame keeps only a complete composite
- **WHEN** the reel strip's and the poster's offline frame is composed with one layer's source absent, and a poster is made of a project whose file is missing
- **THEN** the incomplete frame leaves no picture (no partial cell); the poster of the project with the missing file is the frame with the placeholder, as the preview shows it
- **TEST** t2419 (`src/apps/kt-app/src/reel_preview.rs`)
