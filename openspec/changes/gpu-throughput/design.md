# Design: gpu-throughput

## Context

His trace (run 3 of `fails/26.11.10.2`, copy and analysis in the orchestrator's scratchpad `k3/`): 232 s,
AMD Raphael iGPU (RADV), 16 cores, project at 60 fps (bound 8.33 ms). 1792 near-class admissions waited
70.0 s together, p50 33.7 ms, none of them over the bound by their claims (claim p50 5.9 ms, in flight
before it p50 7.5 ms): every wait is the previous piece's COMPLETION, which stands behind the display's
present — 32.5 ms of Camera ISP + VHS a frame on that card, submitted on the display device with no
admission, so the ring never saw it. Prefetch walks: 9.5 positions/s playing (106 ms wall, 16 ms device, 103
ms admission a position), 7.2 paused. 6 laps to finish the history of a 715-position loop, ~100 positions a
lap. The process used 20 % of one core.

## Mechanism (the class)

**Work a person waits for runs serially while the device or the cores stand idle.** Five forms of it:

1. a waiter admitted only after the earlier piece is DONE, not when the device has room (480);
2. a prediction that does not exist yet, so the safe answer (one pass a piece) is the slow one (481);
3. a look-ahead bounded by the display's reach instead of by what the memory may hold (482);
4. CPU work done on one thread although it splits without changing a byte (the preview decoders' RGBA
   conversion);
5. records that cannot show any of it (486).

## Decisions

### D1. The ring admits by its predicted drain (480)

Every flight carries its predicted device time and, once submitted, its submit instant. The ring simulates
its FIFO: `end_i = max(end_{i-1}, submit_i) + ns_i` (a flight not yet submitted is taken as submitted now; a
flight past its predicted end and still not done ends "now"). The display's next submission is expected at
`last display submit + frame period` while the display presented within the last two periods, else now
(paused, or showing a held picture: a press of Play may come at any moment). A submission of a class below
`now` is admitted while

    drain_end + ns <= max(display_next, display_end, now) + bound

— the lower work it would leave in front of the display's next frame stays within the bound, as before; the
display's own late frame (`display_end` past `display_next`) is its own cost, not the lower class's. `now`
work keeps the old rule (one bound of predicted time in flight, one piece always fits).

- **The display's present is counted, never admitted**: `Ring::note_display(ns)` pushes a `now` flight with
  no wait, submitted with `surface = true`; its prediction is the timer's last measured preview frame (a
  replay of a kept composite: the display pass's own measured time).
- **Submit-ahead**: a lower class's piece on a displayed ring is cut at HALF the bound, so two pieces fit and
  the next is encoded and queued while the earlier runs. On a ring without a display nothing changes (no
  bound, no cuts).
- **A guard on predictions**: at most `AHEAD_PIECES` (3) lower-class flights of one ring are uncompleted at
  once; past it the waiter completes the oldest by the existing blocking wait (never on a surface device).
  A prediction too low then cannot pile work in front of the display.
- **The wait has a deadline**: the condition turns true with time alone (a flight's predicted end passes),
  so the waiter sleeps on the ring's condition variable until the instant its claim fits (or a completion),
  not until a completion only. Before it sleeps it polls the oldest flight's device without blocking, so a
  completion that came is noticed.

Rejected: raising the bound (the display's latency is the owner's limit, t2254); counting the display
without a time model (it would only add waits: the display's 32.5 ms would fill the bound four times over).

What it cannot do: while the display itself saturates the device (32.5 ms of composite every 16.7 ms period
on his card), a lower class still gets about one bound a display frame. The display's composite over a held
picture is removed by `catch-up-shows-loading` (row 483, the owner's 11.11 items 4–5); after it the display
is light during catch-up and D1 hands the device to the walk.

### D2. Cold passes are predicted (481)

- **A disk table of measured pass times** (`kt_gpu::seeds`): `<cache dir>/pass-times-v1.tsv`, one line per
  (adapter name + driver, label, work units) → ns. Labels: a render pass's `PassLabel` name, a compute
  dispatch's name; work units: a render pass's target pixels where the engine knows them, a dispatch's
  workgroups. Loaded once at the first lookup; written by the thread that harvests timings, at most every
  30 s while something changed, and at the engine's drop. A prediction for a size never measured is scaled
  from the nearest measured size of the same label (linear in work units, never below the measured time of
  a smaller size). Keyed by the device's name and driver: a new driver starts empty, a stale line is
  overwritten by the first measurement. A cache: invisible to bytes (cuts change timing only, t2253).
- **A compute job learns while it runs**: at each cut the piece's dispatch timestamps are resolved into
  their own part of the resolve buffer and copied to a small read buffer; when the piece completes, the next
  `due` check reads them and every later dispatch of the same name is predicted from them. The first cold
  walk is then cut by measured times after its first piece.

Rejected: "an unmeasured pass claims nothing" (a cold 300 ms pass would stand in front of the display).

### D3. The history is computed ahead (482, L4)

`stream_ahead_frames` stays the NEAR part (twice the display's reach, class near). A FAR part follows in the
same pass at class `background`:

- with the playhead inside a loop region: to the region's end and round from its start, up to the positions
  the stream memo holds (`size_stream_memo` already sizes it for the loop and its checkpoints);
- without one: ahead of the playhead up to a quarter of the memo, as the near part's bound;
- on a pause inside a loop region: the same far part (the owner's "наперед" — the loop's first lap plays
  computed); on a pause outside one: the frame on screen only, as before.

It is stoppable as the near part (`stop_when`: transport generation, project version), it stops at the first
position whose source is not decoded (the decode order then asks it — the pacing is unchanged), it skips kept
moments, and it is admitted below everything on the ring. At rest nothing runs: a pass with nothing pending
parks the prefetch thread (one-scheduler), and a history complete for the loop leaves nothing pending.

### D4. The processor as a second executor (L4) — decided per kernel

Invariant 11: bytes between runs on one device must not depend on which executor ran a piece. So a CPU
executor may take work only where its bytes are the device's, or by a choice that does not follow the load.

| Work | Decision | Why |
|---|---|---|
| Decoder RGBA conversion of a span (preview) | **CPU threads, as a render** (`rgba_threads` by memory) | the same swscale per picture, split by pictures: bytes equal (t1456 conformance holds threads). The preview converted on the decoder's one thread |
| Datamosh source conversion RGBA→Y'CbCr | decided by measurement (stage 4) | `Planes::from_rgba` is the device's bytes (t2274), so the CPU may take it whatever the load; it pays only if the dispatch is a real share of a position's device time |
| Datamosh clean encoder, residual, decoder | device only | the residual's DCT is f32 with a division (WGSL: 2.5 ULP) and drivers contract multiply-adds: a CPU result differs from the device's, and a load-dependent split would change the bytes. An always-CPU codec is a new look (t1239 pins the bytes) and slower than any card |
| Datamosh GOPs of the clean encoder in parallel | not done | on one queue dispatches run in submission order; batching already removed the round trips |
| Other events' histories concurrently | not done | his project has one stream layer at a time; a second walk on the one compute queue fills only gaps D1 already fills |
| Camera ISP, VHS tape, every barrier | device only | no CPU kernel exists since compute-kernels-render (owner, acceleration scope: the byte reference dropped) |
| Paired audio halves (tape, codec sound) | audio render thread | invariant 2: FX state only on that thread |
| Audio prefetch decode, peaks, thumbnails | already their own threads | see the class walk |

### D5. The rule (485)

A requirement in video-plan: work a person waits for — the catch-up of a stream history, Loading, a paused
frame, a loop's first laps, an export — uses the machine within the limits the owner set: the display first
(its bound), and rest costs nothing (no work at rest, invariant 12). Its measured scenario counts from records,
not wall-clock guesses: a walk with no display traffic leaves the device idle between its pieces less than a
fifth of its wall (`gpu_start` per piece: device end of one to device start of the next), and the walk's
admission share falls under half.

### D6. Records (486)

- **A query set per job in flight** (`compute.rs`): a pool of timestamp sets, made up front (four); a job takes
  one and gives it back; with none free it runs untimed (never a creation per job — t2023). Two concurrent jobs
  no longer resolve each other's timestamps.
- **`gpu_start` per piece**: a piece's submit time, its first dispatch's start, its last's end; `cause` keeps
  the caller's frame. The record keeps its layout (`aux` = the calibration's error in µs); a new event
  `GpuExec` (98) names the executor of each piece (`aux` 0 device, 1 CPU; `a` the piece's index, `b` its
  predicted ns, `id` the ring) — one for every piece the device runs and every CPU span that does a
  device's work.
- **`Edit.a`** = the model's revision the player took (the app passes it with the project).
- `kt-cli trace gpu` prints, per class, the walks' admission share (admission wait / job wall) and the
  device's idle gaps between consecutive pieces.

## The class walk (where work a person waits on runs serial with idle cores or an idle device)

Checked by reading the code paths; every caller of what this change reorders (the ring's admission) listed.

| Path | Found | Done |
|---|---|---|
| Stream history catch-up (prefetch walk) | admission per completion (480), cold claims (481), reach-only advance (482) | D1, D2, D3 |
| Compose thread's stage (≤2 positions) | same ring: a `now` job, old rule | unchanged (now keeps one bound) |
| Camera ISP / VHS composites (offline engine's slow route, `SLOW_CHAIN`) | cut into pieces, waits per completion | D1 (same ring code) |
| Loading on a pause (present of an incomplete frame) | display's present uncounted | D1 counts it |
| Render / export | its ring has no display unless the editor is open on the same card: then D1 | D1; a stream stage still keeps the serial export (row 490) |
| Thumbnails, filmstrips, reel strips, posters (offline engine) | background class on the same ring; per completion | D1 |
| Decoder RGBA conversion (preview) | one thread a source | D4 |
| Pipeline warm (`warm.rs`) | one pipeline at a time after `wait_frames`, by design (never in front of a frame) | unchanged |
| Audio analysis (LUFS/sync/ducking jobs) | scheduler lanes, background, own threads | unchanged |
| Export pre-roll of a stream stage | serial on the compose thread | row 490 (not this change) |

Callers of `Ring::admit` / `Piece`: compute host `run` (every compute job), engine `execute`/`submit`
(offline, export, slow route), the gpu-share bench; `Submitted::wait` callers (`wait_presented`). All take the
new admission without code change; `present`/`present_kept` call `note_display`.

## Risks

- A prediction far below the real time lets lower work run past the bound: bounded by `AHEAD_PIECES` and by
  the next measurement overwriting the seed.
- Two devices of one card are separate queues on the hardware: the FIFO model is an approximation there;
  the latency bench (t2254) and t2253 hold the bound.
