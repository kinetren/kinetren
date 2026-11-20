# Design: track-mosh-and-start-wait

## D1. Why Datamosh on a track is refused, not made to work

A stream stage (`kt-engine/src/video_stream.rs`, datamosh-codec-path D1–D6) decodes an EVENT's own
stream: its source frames from the cache at the codec's size, the event before it on the lane as the
history, another event as the second input. The order asks exactly the frames the kernel names
(invariant 4); the memo keys them by source addresses.

On a track the stage's input would be the track's COMPOSITE: every history position a full
composition of the track's events (their chains, transitions, levels) at a past moment. That needs:

1. a plan that composes PAST moments of a track as the stage's input — a recursion of `frame_plan`
   inside itself (invariant 7 still one path, but a new shape of it);
2. an order that asks, for every history position, every frame of every event on the track at that
   moment (`layers_at` over a window of the past), and a prefetch reach of a whole GOP of composites;
3. memo and durable keys of a composite as a stream input (today a source address and a content
   serial);
4. the stage's `front` (the slots before it) meaning on a track, and the pacing of its history on the
   preview.

That is new architecture, not an executor's change. Until it is designed (spec-engine), the track
window refuses the slot with the reason; its sound half was already left out (track-device-sound).

## D2. One door to the window in the live rig

`ui-probe.window()` exits the process when no window is found — a clock limit of zero. Every call in
`kt_live` (the stand's geometry, `resize`, `focus`, `geometry()`) now goes through `the_window`, which
asks again every 0.1 s while the program lives and fails at once if it ended, up to `TEST_DEADLINE`.
The two waits that were already loops (the window gone after a scenario, a key not read) keep their
`try`. The start waits are bounded by the same guard. t3238 refuses a bare call.
