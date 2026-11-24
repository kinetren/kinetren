# Proposal: gpu-waits-named

> Found-defects 835, taken by the orchestrator's order (2026-11-24). Tests t3250–t3253.

## Why

In the second stage-24 finish a composite of the preview and a frame of the render-ahead producer stood
4.8 s with no processor work and ended in the same two milliseconds. The trace had two spans with nothing
inside them: it did not name one wait between a frame's start and its admission on the ring.

The mechanism: **work for a picture calls the disk while it holds what the next frame needs.**

## What Changes

- The trace names every wait of a frame's device part past half a millisecond (`gpu_wait`): a lock or a
  turn somebody else held, one call into the device (pipeline creation, upload, uniforms, submission,
  poll, the wait for a submission and its map), a file. `kt-cli trace frame` prints them inside the
  executor's span; `during` sums them by name.
- The pass-time table is read and written by a short thread of its own, never with the table's lock held
  (the cause).
- The displayed frame waits for a moment of the disk tier at most a bound, and asks none while an earlier
  read stands.
- A compute pipeline is created under its own text's lock, not the table's of all texts.
- The player's executor and the disk tier's store are dropped outside the locks the preview takes; the
  store's reader is joined off the UI thread.
- Two windows that asked the disk in every drawn frame ask it once.

## Capabilities

- `telemetry`, `video-plan`, `durability`.
