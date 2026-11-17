## Context

`docs/architecture-review-26.11.17.md` §1–§2 and R2/R5. The preview while playing was a latency system: a
compose thread composing the frame asked, one at a time, latest-wins; loop-fill lanes composing a loop's
unshown moments on other engines, each moment a chain of round trips (admission of pieces at half the
bound, then a readback, then an upload into the display's memo); a ring that held half a frame period of
lower work in front of the display, at most three lower pieces; a far stream walk with a stall counter.
Each rule was tuned on llvmpipe in a VM and right for one trace. Together they kept about one frame in
flight: on the owner's iGPU (kt26.11.16, `rendereg_2`) a fill moment took ~174 ms of wall for ~12 ms of
device, the card was ~26 % busy, and a VHS Tape loop the card could compose at 60+ frames a second took
seven laps.

The owner's decisions (2026-11-17, `kkt-26.11.17` S4): the half-frame bound is raised; his own rules stay —
no partial picture (kkt-26.11.14 M1), audio is the master clock (invariant 6), one composition path
(invariant 7), the UI never waits (invariant 12).

## Goals / Non-Goals

**Goals:** playing, the display presents finished frames only, and one producer keeps the device fed with
several moments in flight, measured, not tuned; the loop fill, its lanes and gates, the arbiter's
half-frame bound, its rooms and piece count, and `FAR_STALLS` are deleted, not layered over; acceptance
reads utilisation (the device's busy share while frames are missing).

**Non-Goals:**
- the compute host (Datamosh) on the display's device. It stays a device of its own: the window's device
  is created by eframe (on GL with downlevel limits, no compute) and the compute host needs Vulkan compute
  with its own features; sharing them is a change to eframe's device creation, testable only on a card.
  Rows 663 and 693 stay open; the seam is `VideoGpuExec::ahead` (a held frame reaching another engine of
  the same device on the device) and the ring, which already orders by class.
- a disk tier for kept composites (`render-cache-disk`), a processor executor (`cpu-second-executor`), the
  export pipeline, the export's device count and the render dialog (`render-honest`).
- a "rendering ahead N %" text under Loading: Loading reads as before.

## Decisions

### D1. Playing, the display presents; a producer composes

While the transport plays and the producer has an executor (and `KT_RENDER_AHEAD` is not `off`), the compose
thread asked for a moment shows its kept composite (`show_kept`, the probe plan, no decode, no stage) or
nothing: the answer is incomplete with no picture, the preview keeps its last full picture under Loading
(no partial picture), and the producer is kicked. The on-demand composite stays where the person drives the
moment: paused, scrubbing, seeking, the bytes path (the GL preview off the window's device, whose ask has
no present target), and a player whose producer has no executor (tests, a headless bench without one).

Why not keep both: two producers of the same moments race for the device, and the display's on-demand
composite is exactly the one-in-flight round trip the review measured. Rejected: "display composes when the
producer is behind" — the frame is late either way, and composing it twice costs the frames after it.

### D2. One producer, one order

`render_ahead.rs`: one `near` service lane. A pass looks from the frame on screen plus the producer's
measured lead (the wall a moment takes, at least one frame) forward: in a loop region round the region to
the moment before the lead (every moment of the lap), outside one forward until the first moment whose
layers are not all decoded (the look-ahead's window, sized by the memory ledger, is the horizon — no
constant). It skips moments the display holds (noting them for the bars), moments skipped this lap
(incomplete — a stream stage short — or refused by the memo), and composes the rest in that order. It asks
no decode and withdraws no request (the prefetch thread owns both). Nothing runs while nothing plays.

### D3. On a card, the moment stays on the device

`VideoGpuExec::ahead(plan, keeper)`: compose and keep the composite on the keeper (the display's executor)
under the plan's key. kt-gpu: when the keeper is a `GpuFxEngine` on the SAME device (the producer runs on
the display engine's `sibling`), the composite texture is submitted and inserted into the keeper's present
memo at once — the queue orders the keeper's later display pass after it — and the call returns a done flag
the device's work-done callback sets: no readback, no upload, no wait. Otherwise (another device, a software
rasterizer) it composes and reads back (`execute_into`) and the keeper uploads (`keep`): the old slow-route
form, synchronous. The bytes of a kept composite are the bytes `present` would have kept: the same resolve
and passes (not the export's settled form), and nothing is kept when a run's form moved while it resolved.

Why the display's device: the review's "one device per card". On a card every device we open runs through
one graphics ring anyway (gpu-arbiter); the offline device only added the readback and the upload. On a
software rasterizer each device is a queue thread of its own, and the display's thread is the one that
must present; there the producer keeps the offline device's side engine (scheduler-followups D5).

### D4. Depth from measurement

The producer keeps at most `depth` moments in flight: `ceil(lat / own)`, where `lat` is the EMA of a
moment's wall from the start of its plan to its done flag and `own` the EMA of the producer thread's own
time per moment (plan, encode, submit), at least one, and at most the moments whose bytes the memory ledger
has left (`kt_time::mem::room()` over the moment's bytes; unknown room — no cap beyond the measured one).
Little's law: fewer leaves the device idle while the thread waits; more than the thread can feed buys
nothing. A synchronous moment (D3's bytes form) has `lat == own`: depth one, by measurement. When full the
producer waits for the oldest moment's flag, polling the device without blocking (`poll_done`), never on a
surface.

### D5. The arbiter's bound is a frame period (R5)

`arbiter::bound()` = the preview's frame period (was half). A lower class is admitted while the predicted
drain ends within the bound past the later of the display's next present and the end of its frame in
flight — every lower class alike: the `room` rules (a quarter bound, an export's slack: gpu-leftovers D4,
loop-laps-regression D2) and the count `AHEAD_PIECES` are deleted. A lower piece is cut at the bound (was
half of it). Raising it lets lower work delay a displayed frame by up to one frame period; the owner
decided that ("Поднять"). Correction of record: video-plan design's "Rejected: a larger bound (the owner's
latency limit)" and loop-laps-regression D2's "The owner's latency limit stays" named a rule he never set;
`gpu-arbiter` D3 called it a general rule he may revisit. Both are amended in the merged design.

Why one period and not K frames: the display while playing shows kept frames (a display pass, well under a
millisecond on a card), so a period in front of each present is a period of producer work queued at every
moment — the device never runs dry while the producer keeps up. A longer bound would only hold the frame
on screen longer when the person seeks.

### D6. `FAR_STALLS` → the cache's decoded count

A far walk that walked no position waits until the frames cache has gained a decoded video frame
(`BurstCache::video_inserted`) or the pass state moves; the eight-pass counter is gone. A source that never
comes costs no pass; one that comes wakes it at its first frame.

### D7. Utilisation is the acceptance

`kt_gpu::pass_time` writes a `GpuFrame` record per timed frame (`a` its device ns, `aux` the role) when its
timestamps come back. `kt-cli trace loop` prints per lap the device's busy share (device ns harvested in the
lap over its wall) and, over the laps that had a moment not kept, the share "while frames were missing".

## Tests the owner's decision rewrote

The break was decided by the owner (2026-11-17). Each test that pinned the old mechanism:

| test | was | now |
|---|---|---|
| t2334 | the loop fill makes a loop shown in part all-kept | the producer does (`tests/render_ahead.rs`) |
| t2335 | the fill asks no decode, rests with the player | the producer does |
| t2336 | the fill stops where the memo stops keeping | the producer skips refused moments for the lap |
| t2860 | several fill moments at once, each on its own engine | REMOVED: replaced by t2920 (moments in flight on one engine, done by the device) |
| t2861 | the width from the pool and `RING_LOWER_PIECES` | REMOVED: replaced by t2921 (depth from measured wall and room) |
| t2862 | an incomplete moment narrows the fill | REMOVED: no width to narrow; the moment is skipped for the lap (t2336's rule) |
| t2430 | at most three lower pieces of a quarter bound | lower pieces admitted while the drain fits the bound, no count |
| t2433 | lower work in front of the display within the half-frame bound | within one frame period |
| t2517 | the room by the display's cost (quarter bound, export slack) | REMOVED with `room`: replaced by t2923 (every lower class keeps the bound) |
| t2251, t2431, t2441, t2865 | the bound | the same rules at the new bound (numbers follow `bound()`) |
| t2432 | the far walk reaches the loop's end | the same, and a stalled walk waits for a decoded frame (t2924) |

t2020–t2025 (pass timer, no creation per frame) were read: they pin no scheduling and stay as they are; the
on-device keep creates only the memo's texture while it fills, as t2023 allows.

## The class walk (a problem is a class)

The mechanism: work that the device could run in parallel is serialized by a round trip per item (admit →
submit → wait → read back → hand over) with one item in flight. Where it acts:
- the loop fill — replaced (D2–D4);
- the display's composite while playing — replaced (D1);
- the arbiter's bound and piece count — D5;
- the far stream walk's retries — D6;
- the slow-chain route (pause, scrub): one frame the person waits for, latency-bound by nature — kept;
- reel strips, posters, effect thumbnails on the side engine: offline jobs of the visible/background
  classes, one frame each — kept, they benefit from D5;
- the compute host's Datamosh jobs: a device of its own (Non-Goals) — open, rows 663, 693;
- the export pipeline: already a ring of three frames in flight (`render-honest` owns its device count);
- the kept-moment recheck: plans only, no device — not this class.

## Risks / Trade-offs

- [A cold heavy region shows Loading at the start of play until the producer reaches the playhead] →
  the producer's first moment is the frame on screen; the owner accepted a later start for a filled
  pipeline (review R2).
- [A seek lands on a moment not kept] → playing, the producer starts there at once (the kick); paused, the
  on-demand composite runs as before.
- [Bytes kept by the on-device path differ from the bytes path] → both are `present`'s resolve and passes;
  t2922 holds them equal.
- [llvmpipe cannot show the card's gain] → the bench shows the software case; the card case waits for the
  reference mini-PC (owner, 2026-11-17). Stated in the measurements.

## Measurements

(filled by stage 5)
