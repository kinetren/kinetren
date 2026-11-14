# Proposal: no-partial-picture

## Why

The owner's answer to found-defects 461 (`kkt-26.11.14` M1, verbatim): "пустое, нигде не допустима
частичный показ и введение в заблуждение, частичный показ - это опять лишняя работа по отрисовке!!".

The first picture of a fresh preview was presented incomplete under the Loading scrim
(`show_incomplete: !shown_any` in kt-app): layers absent, the later stages run over what was there. The
scrim itself was drawn only over a picture, so a preview with none had to show the partial one to carry
it. And every incomplete composite, presented or not, was resolved, uploaded and encoded; on the bytes
path (the slow-chain route, the loop fill, reel strips and posters) it was also read back, then thrown
away.

## What Changes

- **No incomplete picture is ever presented.** `PresentOptions::show_incomplete` is gone: an incomplete
  frame never touches the display target. The app draws only complete composites; until the first one
  lands the preview's place is empty, and the Loading scrim and its text stand in the frame's place over
  it (the scrim moved out of the "there is a picture" branch).
- **An incomplete frame costs no drawing work.** `FramePlan::starved()` says, before anything reaches the
  device, that a layer at any depth lacks its frame. kt-gpu's `present` and `execute_into` stop there
  (no upload, no pass, no readback); an incomplete plan found only by its resolve (a foreign size, a
  reel's sub-plan) stops after the resolve. `execute_into` leaves an incomplete frame's bytes empty; only
  `execute_once` keeps them, for the export's last resort.
- **The meter under the cursor shows no level of a partial mix** (found by the class walk):
  `Player::sample_levels_at` returns `None` when a burst it renders is not cached (it read the missing part
  as silence and showed that level); the meter falls instead.
- `kt-cli gpushare` counts a latency only for a frame it actually drew.
- The live scenario `open_heavy` checks that no picture is drawn under Loading on a fresh open (red on the
  old build: 29 looks) and prints the open's work and the pause after it.

## Capabilities

### Modified Capabilities
- `video-plan`: the display target and the bytes path never carry an incomplete frame, and do no work for
  one; the "first picture" exception is removed.
- `ui-timeline`: a preview with no complete picture is an empty place under Loading; the scrub meter gives
  no level for a partial mix.

## Impact

- `kt-engine`: `video_plan.rs` (`FramePlan::starved`, `PresentOptions`), `player.rs`
  (`sample_levels_at` → `Option`), `preview_bench.rs`.
- `kt-gpu`: `engine.rs` (`present`, `execute`).
- `kt-app`: `app.rs` (`draw_preview`), `meters.rs`.
- `kt-cli`: `gpushare.rs`.
- Tests: t1526 rewritten, t1527 adjusted (a new size comes with a complete frame); t2530–t2534 new.
