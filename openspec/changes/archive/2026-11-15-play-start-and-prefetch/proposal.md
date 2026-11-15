# Proposal: play-start-and-prefetch

## Why

Two rows of `docs/found-defects.md`, given to one change by the orchestrator:

- **525** — `kt-engine/tests/loop_fill.rs` t2335 ("the prefetch thread stopped passing") waited its
  whole deadline under a loaded machine (1 of 4 runs beside 48 busy loops, once in a full parallel
  kt-engine run); alone it passed in 2 s. A lost wakeup in the prefetch path would mean playback that
  stalls on a busy machine.
- **516** — the delay of opening the sound device on Play was unmeasured: `preview_bench` modelled an
  always-open device, so `kt-cli preview` never saw the 7–20 ms (seconds under load, row 425) that
  kept-mix-settle added. Play should not wait for it if it is long (invariant 6: the callback is the
  master clock).

## What Changes

1. **Row 525 diagnosed** under gdb with 48–96 busy loops on 32 threads: the hang was the test's own
   race — it read the pass counter AFTER the seek that owes the pass — already fixed by ec6be337
   (followups-races) on a branch parallel to the one where the row was recorded. The pre-fix test
   hangs 8 of 12 runs at load 48; the current one passed 42 of 42 at loads 48–96, never waiting over
   30 s for a pass.
2. **The class walk of thread hand-offs** (design D2) found three of the same kind and fixes them:
   - the compose thread's stop notified without the lock: a stop between its look and its wait was
     lost and the thread (not joined) stayed parked holding the player's preview parts;
   - a seek's `flush` races the start's pass: the consumer's drain could drop the pass the render
     thread had just written, and the start lost its first ring of sound (the flush is now stored
     before the generation, and a start holds while a flush it must wait for is pending);
   - the trace writer's park handshake lacked the producer's SeqCst fence: a record could wait in
     the ring until the next one.
3. **Play's first sound is measured**: the render thread writes `play` aux `first_sound` (µs from
   Play to the first frames a callback handed out) and keeps it in `AudioShared::first_sound_us`;
   `preview_bench` has a device double that opens `device_open` after Play; `kt-cli preview
   --device-open-ms N` prints "play → first sound"; `kt-cli trace summary` prints the first sounds,
   the device's opens and resumes.
4. **Play does not reopen the device**: at rest the stream is paused, not dropped (measured: a paused
   cpal/PipeWire stream wakes nothing — 0 callbacks, the same context switches as a closed one), and
   Play resumes it in 20 µs instead of an open of 28–112 ms under load. The first Play still opens it
   (the owner's "open the sound on Play"). A backend whose pause fails or keeps calling back is closed
   as before; a seek while paused is drained by a short resume.

## Capabilities

- `transport` — MODIFIED "The sound device is open only while there is sound to play"; ADDED "A
  start's pass waits for the drain of its flush", "Play's first sound is measured", "A stop or a seek
  handed to a parked thread is never lost".

## Impact

`kt-engine` (player, compose_queue, preview_bench), `kt-time` (trace), `kt-app` (audio_out),
`kt-cli` (preview, trace summary), live `idle`. No model or file change; nothing for OTIO.
