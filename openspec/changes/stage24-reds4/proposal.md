# Proposal: stage24-reds4

> Found-defects 842 and 843, the reds of the third stage-24 finish (orchestrator's order, 2026-11-25).
> Tests t3262–t3266.

## Why

843: an effect cell of lesson 2 panicked in a debug build, and every later cell of the session panicked on
the lock the panic poisoned. What the panic guarded is a defect of every build: a layer whose decode step
nothing in its chain brings back has a size of its own, and a layer of another size is never drawn. A zoom
animation that stands at exactly 1, and a Datamosh behind a zooming Geometry, showed Loading and no export
wrote their frames — on a source with at least twice the preview's detail.

The mechanism: **the decode step is asked of the clip over its whole length, and used at a moment whose
chain may not consume it.**

842: after "Next lesson" lesson 5 opened 31.8 s later. The trace names it: the save job of lesson 4's copy
ran 31 797 ms (an fsync on a saturated disk), and the next lesson waited for it.

The mechanism: **a gesture waits for the durability of a file that is the program's own.**

## What Changes

- The chain's planner brings a frame nothing consumes back to 1x first, through the identity placement.
- The slots in front of a stream stage ask nothing of the layer's decode step.
- A panic in one item of a background queue is that item's failure; the run goes on with the rest.
- A lesson's copy is written and read with no fsync and no wait, its text readable on its way.

## Capabilities

- `video-plan`, `ui-controls`, `media-index`, `durability`.
