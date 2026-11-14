# Proposal: stream-memo-budget

## Why

class-walk-revision recorded what stages 9–11 left in the stream memo's budget and in the jobs a
playback's vote holds (`docs/found-defects.md` rows 512–515, 517–519). The owner (kkt-26.11.11): fixes
are checked in the other places too; diagnosed defects are fixed without asking.

- 512: the memo's checkpoint tier was sized without the stages inside reels.
- 517: the stream memo's LRU kept nothing of a loop longer than itself — every lap recomputed every
  position — while the present memo keeps a part (memory-ledger D5).
- 518: the want counted every moment of the loop region at the size of the frame on screen: a short
  Datamosh in a long loop over-asked History (which the ledger grants before Frames), and a stream
  event of another size re-sized the memo at each crossing.
- 519: the reel strip's and the poster's renderers filled the preview's memo, which is sized for the
  player's loop and does not count them.
- 514: their stream pre-roll ignored the pause vote inside one frame.
- 513: the insurance autosave, due after two intervals of playback, was posted on the Background lane
  the player's vote holds: nothing reached the recovery file until a pause.
- 515: "no thumbnail while playing" was proven by a 300 ms sleep.

## What Changes

- `StreamMemo::keep_loop` / `lap`: told of a loop, an entry a recent lap used is not evicted for a new
  one; new entries pass through a window of a quarter of the bound (t2490).
- `VideoRenderer::stream_want(from, to, q)`: every stream stage over the region at its own size, reels
  included, with each walk's checkpoints; `stream_checkpoints` descends into reels (t2491, t2492). The
  player sizes the memo by it once per build, loop and quality, tells the memo the loop and marks a
  lap when the shown moment goes back inside the loop while playing.
- The strip and the poster build their renderer with a memo of their own (`offline_renderer`, t2493)
  and run the pre-roll under a stop check that answers the hold, waiting between batches (t2494).
- The insurance autosave goes on an Answer-class lane; a Background autosave a playback holds for
  another interval is taken by that lane (t2495).
- t2238 proves the pause by the scheduler's order and counters.
- Tools: `kt-cli stream-cost` sizes and laps as the player; `gpu-share`'s offline thread has its own memo.

## Impact

kt-plugin-api `stream.rs`; kt-engine `video.rs`, `video_stream.rs`, `player.rs`; kt-app `reel_preview.rs`,
`bg.rs`, `app.rs`, `thumbs.rs`, `ui_wait_tests.rs`; kt-cli `streamcost.rs`, `gpushare.rs`. Specs:
video-plan, compositions, durability.
