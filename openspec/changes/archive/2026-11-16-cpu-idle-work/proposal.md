## Why

The owner (`kkt-26.11.15` item 4, line 7): "по прежнему не понимаю почему не используется cpu при работе
программы - он тупо простаивает по большей части". His `rendereg_2` trace (kkt-26.11.16b) shows why: while a
loop fills, the process holds 13 % of ONE core of his sixteen, and the card is busy about a third of each
second (20 frames of ~15 ms). Neither resource is the bound: the work is a chain of round trips run one
at a time. One fill moment takes ~170 ms of wall for ~20 ms of device time (seven pieces, each admitted
after the one before, then the readback), only one moment is ever in flight, and no moment starts while
the display composes. Throughput is the bound of what is in flight divided by the round trip — Little's
law — and the in-flight amount is one.

## What Changes

- The loop fill runs several moments at once on a hardware device: as many as the scheduler's pool
  leaves beside one worker for a person's answer, at most the three lower-class pieces the ring admits in
  front of the display (`AHEAD_PIECES`). Each runs on its own engine (a sibling of the fill's executor:
  same device, ring and compiled pipelines, its own lock), and no two take the same moment.
- On a hardware device a fill moment no longer waits for the display to finish its own composite: the
  ring's arbiter already puts the frame on screen first. On a software rasterizer the display and the fill
  share the processor; there the fill stays one moment at a time behind the display (the measured rule of
  scheduler-followups).
- A fill moment that comes out incomplete (a stream stage short of its history) narrows the fill to one
  moment for the rest of the lap, so parallel fills never take the device from the history walk the display
  needs.
- `KT_LOOP_FILL_WIDTH=N` overrides the width (an A/B of one binary on the owner's machine).
- The trace names each fill lane, so `kt-cli trace loop` shows how many ran at once.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: the requirement "A playing loop's unshown moments are composed below the display" changes
  from "one at a time, never while the display composes" to a width sized from the machine and the
  device kind.

## Impact

- `src/libs/kt-engine/src/loop_fill.rs` (lanes, claims, width), `video_plan.rs` (`VideoGpuExec::sibling`).
- `src/libs/kt-gpu/src/engine.rs` (`sibling` through the trait).
- No change to the arbiter, the export pipeline (`render-speed` owns it) or the render dialog.
- Tests: `src/libs/kt-engine/tests/loop_fill.rs` (t2860–t2862).
