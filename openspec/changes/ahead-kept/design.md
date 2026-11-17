## Context

The owner's trace `fails/26.11.18/new` (kt26.11.17-2abc3af9, the first release with the render-ahead
pipeline; Ryzen 7700X, RADV RAPHAEL iGPU, preview 640×360) plays an 11 s loop of one event: Camera ISP in
front of Datamosh, VHS Tape after it. `kt-cli trace … loop` reads 17 laps; the loop is whole from lap 16;
the device is busy 37 % while frames were missing; the producer keeps 6–92 of the 70–352 moments it
composes a lap, "up to 2–3 in flight".

What the records say (`trace … dump`, counted by a script):

- **Every unkept moment of the producer is a short history.** 2904 render-ahead jobs: 588 kept, 2316
  `b = 1`, and every one of those has exactly one child record, `stream_run` with `aux = SHORT` (the history
  19–64 positions behind the moment). None was refused by the memo, none evicted (`memo_evict` 0).
- **The work is the history.** A history position runs the front (Camera ISP at the codec's size: 4 pieces
  of 15.6, 16.4, 25.7 and 1.2 ms, `GpuFrame` 59 ms of device) and a Datamosh job (7.8 ms); the composite
  after it is 23 ms. 690 distinct positions were walked in the run, 27 walked twice: nothing is walked in
  vain, the loop needs ~40 s of device for its history.
- **The device idles between pieces.** A history position's `GpuFrame` span is 117–124 ms for 59 ms of
  device. Its pieces wait for admission 33 ms at the median (`gpu_admit`, the prefetch thread's `now` and
  `near`/`background` walks, the producer's `near` moments: 94 + 46 + 49 + 15 s of waits in a 160 s run).
  Each claim is the whole bound (16.67 ms: the claim of a piece is capped at `piece_limit` = the bound) and
  a `now` piece is admitted while `left + claim ≤ bound`, a lower one while the drain plus its claim ends
  within the bound past the display's next present — which is NOW while the display presents nothing (the
  frames are missing). A claim of the whole bound therefore fits only an empty ring: each piece waited for
  the one before to END, and the device stood idle while the waiter noticed it, encoded and submitted the
  next. A `now` piece waiting holds every lower class too.
- **The depth stays 1–3** because the producer's own time for a moment includes the history it walks
  synchronously (the stream stage runs in the plan; the Datamosh job waits for its buffers): `own ≈ lat`,
  and the depth rule answers right. It is not the limit: the device is.

## Goals / Non-Goals

**Goals:** the device stays fed while frames are missing; a moment whose history arrives is composed in the
same lap; the next trace says both facts per lap.

**Non-Goals:** the front's cost at the codec's size (Datamosh's `lines` is the person's choice, and the
codec codes that size); a second executor; any change to what a frame is (no bytes move).

## Decisions

### D1. An incomplete moment waits for what it lacked, not for the lap

`render_ahead::Waits`: a try that came out incomplete records the process's walked history positions
(`kt_plugin_api::stream::positions_walked`) and the frames cache's decoded count
(`BurstCache::video_inserted`). Short of `k` positions with a composite computing `s` itself (the pacing's
`compose_steps`), the moment is tried again once `k − s` more positions were walked — a necessary
condition, so a retry is never wasted on a history that cannot have arrived (the `needs` count of a 64-short
moment took 84 ms of the processor in his trace); otherwise once a walk or a decode moved. The `b` of its job
record is 4 when the history was short. A new lap clears the waits as it cleared the skips: never later than
before, should a walk ever move a history without counting its positions. A waiting moment is composed on
demand by the display, as a skipped one was (t2044's cold mosh loop with a 4 MB cache relies on the display's
own composites). The memo's refusal stays skipped for the lap (t2336): memory does not move by waiting.

Rejected: dropping the skip and retrying every pass — each try of a short moment computes `needs`, and the
producer would spin through the lap's hundreds of short moments at the rate the walk advances.

### D2. A piece is half the bound

`Ring::piece_limit` = `bound() / 2`: a frame or compute job is cut where its measured passes would pass half
the bound, and no admission claims more (`Piece::begin`, `claim_after_cut` cap at it). A piece of the
largest size then fits a ring that has another of that size in flight, so the next one is queued while the
first runs. What a lower class may leave in front of the display's next present stays the bound, and a
`now` piece still keeps one bound in flight: render-ahead D5's raise (the owner's "Поднять", kkt-26.11.17
S4) is kept. A single pass longer than half the bound is still a piece of its own (Camera ISP's 25.7 ms
pass at the codec's size): it claims half the bound and is admitted behind the tail of the piece in front,
not after an idle device.

Rejected: a larger bound — not needed to feed the device, and it would hold the frame on screen longer when
the person seeks (render-ahead D5's own reasoning).

### D3. The trace says it

`trace loop` prints the producer's outcomes per lap — kept, short of the history, incomplete, refused by
the memo — and "ring waits X s over N" (the `GpuAdmit` spans). His trace needed a script over the dump to
learn both.

## Class walk (owner, R.8 item 17): every path where work is composed and then not used

- **render-ahead producer** — incomplete moments (D1); memo refusal: kept skipped, memory-bound by design;
  `Err` (too large, poisoned): skipped, no picture exists.
- **display's on-demand composite** (`compose_queue`) — his trace: 1609 asks superseded before composing
  (no work), 119 finished frames never taken (`UNTAKEN`): complete ones are kept in the present memo, so
  their work is reused; 1 with no picture.
- **prefetch stream walk** — 690 distinct of 717 positions walked (27 twice: memo evictions of a loop whose
  history does not fit); 7 stopped walks (`StreamStop`, a newer ask): a stop discards at most one batch.
- **ring admission** (D2) — every bounded ring: the card's (display, offline, side and producer engines),
  the compute host's, a software device's with the slow route; an export's ring is not bounded and keeps its
  ring of three.
- **export** — an incomplete frame is composed again up to `RETRIES` before it is written (gpu-leftovers
  D5): needed, rare; not touched (`render-honest`/`export-readback` own the export).
- **effect thumbnails, reel strips, posters** — offline one-frame jobs on the side engine; reel previews
  are kept by `content_hash` with a disk tier: nothing composed is thrown away.
- **kept-moment recheck** — plans only, no device.
- **slow-chain route** — its bytes are kept on the display's executor (`keep`), reused.

## Measurements

Release, llvmpipe (no card in this machine: a VM), the machine shared with other sessions. `kt-cli showcase
--heavy --mosh-behind` in `/dev/shm`, the 8 s Datamosh event at 62.4 s, `preview mosh.kt 63 67 --size
640x360 --loop 40`, then `trace … loop`. Base = orch `2d1cc934`.

| | base | new |
|---|---|---|
| first lap with every frame kept | 8 | 7 |
| device busy while frames were missing | 67 % | 88 % |
| ring waits, laps 2–6 | 0.3–0.5 s a lap | 0.0 s a lap |

On llvmpipe the device IS the processor (15.9 cores busy in both runs): the share cannot show the card's
idle gaps, and lap counts move by one. The card's mechanism is held by t2930 (a whole piece is queued behind
the one that runs) and t2931 (a short moment is tried again when its history arrives). Predicted for his
card: a history position's span from ~120 ms to ~60 ms of device, the loop's ~40 s of history plus 23 ms a
composite in about 4–5 laps instead of 16 — his next trace reads it (`trace loop`: device %, ring waits, why
moments ahead were not kept).

## Risks / Trade-offs

- [Twice as many pieces on a displayed ring] → a piece is one submission; his heaviest frame is 4–6 pieces.
- [A waiting moment the walk never counts] → a new lap clears the waits; the display composes it on demand.
