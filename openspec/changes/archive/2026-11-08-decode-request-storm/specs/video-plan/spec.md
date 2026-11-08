## ADDED Requirements

### Requirement: A live preview paces a stream stage's history
In the live preview (`StreamPacing`, set by the player on its renderers; `None` for an export and every
other path, whose bytes do not move) the order of a stream layer SHALL be the earliest positions its
history lacks that fit a quarter of the video cache, at least one, in the order the stage reads them. The
prefetch thread SHALL advance the stream stages over what the decoder brought — at the frame on screen and,
while playing, ahead of it (wrapping in the loop region) by the frames the display reaches before its next
pass, twice over, bounded by a quarter of the stage's memo — and a pass that leaves a stage pending SHALL
run again even on a pause. A composite SHALL run a stage only when its frame lacks at most two positions:
on a pause it reports the frame incomplete; while playing it shows the latest frame the stage holds on that
frame's stream, and such a composite is not kept as the moment's own. An offline path with its own sources
and a small cache (the reel strip, the project poster) SHALL pre-roll a stream stage as the export does, in
batches of half its cache.
Why: ordering the whole history (up to 600 frames of 8 MB at 1080 lines against a 512 MB cache) let the
decoder run through it while the stage read only at composites; the cache dropped the history's front, the
frame never came, and the owner saw a loop at V-fps 0. With the history cached, the composite computed it
on the UI thread: 9 s for one composite on his relinked project (decode-request-storm).

#### Scenario: The paced order is the earliest of the history
- **WHEN** a frame 3 s into a cut mosh (a ~100-frame history) is ordered with budgets of 1, 7 and 30 frames, and of one byte
- **THEN** each order equals the first 1, 7, 30 and 1 entries of the unpaced order
- **TEST** t2042 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The advance computes the history, not the composite
- **WHEN** every frame of that history is cached and a paced renderer composes the frame, then runs `stream_advance`, then composes again
- **THEN** the first composite is incomplete and the memo untouched; the advance reports nothing pending; the second composite is complete and byte-identical to the unpaced renderer's; the next frame, one step on, is the composite's own
- **TEST** t2043 (`src/libs/kt-engine/tests/decode_storm.rs`)
