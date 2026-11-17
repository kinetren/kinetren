## Why

The owner is tired of recording a trace for every slowdown (`kkt-26.11.17` S3) and asked whether the
architecture is wrong. The read-only review `docs/architecture-review-26.11.17.md` answered: the computation
is sound, the layer that decides WHEN device work is submitted is not. While playing, the preview composes
one frame at a time on demand, holds at most half a frame period of work in flight, and spreads the rest
over loop-fill lanes whose every moment is a round trip (admission → pieces → readback → upload into the
display's memo). On his Ryzen 7700X with a 2-CU Raphael iGPU the card is 4–26 % busy while frames are
missing; by Little's law, throughput is the work in flight divided by the round trip, and the work in
flight stays near one frame. A loop that the card could compose in real time from lap 1 (VHS Tape at
640×360, ~12 ms of device a frame) took seven laps.

His answers of 2026-11-17 (`docs/ktprompts/kkt-26.11.17.txt`): the half-frame bound is RAISED (it was
never his rule, though two designs called it "the owner's latency limit"); a background disk prerender, a
CPU second executor and a reference mini-PC come after. This change is the first and central one: the
review's R2 (playback as a render-ahead pipeline) and R5 (lift the half-frame bound).

## What Changes

- **BREAKING (internal):** while playing, the preview's display only PRESENTS finished frames. A
  render-ahead producer composes the moments ahead of the frame on screen — the frame on screen first, then
  ahead, in a loop the rest of the lap — with several moments in flight, and keeps each in the display's
  present memo. A moment not ready is Loading (no partial picture, kkt-26.11.14 M1); the on-demand
  latest-wins composite stays for paused, scrubbing and seeking, and where no producer runs.
- The producer composes on the DISPLAY's device (a sibling engine: its own lock, the
  same device, queue and pipelines), and the composite goes into the display's memo on the device: no
  readback, no upload, no wait. Its depth (moments in flight) is measured: the wall a moment takes from
  submit to done over the producer's own time per moment, bounded by the memory the ledger has left. No
  constant of one machine (owner rule H13).
- On a software rasterizer too (measured: the side device's readback and upload cost more than its own
  queue thread gave); the side engine of the offline device stays for a display with no sibling and as the
  A/B `KT_RENDER_AHEAD_DEVICE=side`.
- **BREAKING (internal):** the loop fill (`loop_fill.rs`: lanes, width, claims, narrow mode, the display
  gate, `RING_LOWER_PIECES`, `KT_LOOP_FILL*`) is DELETED; the producer replaces it in loops and linear play.
- **BREAKING (internal):** the arbiter's bound is ONE frame period (was half); every lower class keeps the
  bound in front of the display's next present (the `room` slack rules and `AHEAD_PIECES` are deleted);
  a lower class's piece is cut at the bound (was half of it). The wording "the owner's latency limit" is
  corrected in every design that used it.
- The far stream walk's `FAR_STALLS` retry counter is deleted: a far walk that walked nothing waits until
  the decoded-frame cache gains a frame or the state moves.
- Telemetry: every timed preview frame is a `GpuFrame` trace record (its device time), and `kt-cli trace
  loop` prints the device's busy share per lap and while frames were missing — acceptance by utilisation,
  not by a vanished mechanism.
- Seams for the next two changes: `render-cache-disk` takes the producer's moments and kept composites;
  `cpu-second-executor` adds an executor to the producer's set.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: the loop fill is replaced by the render-ahead producer; the arbiter's bound, rooms and
  pieces; the far walk's retry rule; the display presents only finished frames while playing.
- `telemetry`: `GpuFrame` records and the per-lap device share of `kt-cli trace loop`.

## Impact

- `src/libs/kt-engine`: `render_ahead.rs` (new), `loop_fill.rs` (deleted), `player.rs` (producer wiring,
  prefetch kick, far walk rule, `compose_ahead`), `compose_queue.rs` (present-only while playing),
  `video_plan.rs` (`VideoGpuExec::ahead`, `as_any`, `poll_done`), `tests/loop_fill.rs` →
  `tests/render_ahead.rs`.
- `src/libs/kt-gpu`: `arbiter.rs` (bound, rooms, pieces), `engine.rs` (the on-device ahead path),
  `pass_time.rs` (`GpuFrame`).
- `src/libs/kt-cache`: a count of decoded video frames inserted (the far walk's retry rule).
- `src/libs/kt-time`: `Ev::GpuFrame`, `job::RENDER_AHEAD`.
- `src/apps/kt-cli`: `preview` (producer lines), `trace loop`/`gpu` (device share).
- `src/apps/kt-app`: the side-engine setter renamed; a live scenario for loop playback.
- Not touched: the export pipeline, the export's device count and the render dialog (`render-honest`),
  decode colour conversion (`source-colour-matrix`).
