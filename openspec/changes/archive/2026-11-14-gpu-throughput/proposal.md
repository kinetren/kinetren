# Proposal: gpu-throughput

## Why

The owner's stage-10 run (`kkt-26.11.10b` K3: "он херачит catching up, при этом сожрал 3гб кэша, и при этом
не грузит проц - ленится как будто опять или упирается в одно ядро иногда, у нас нет спек на эту тему??";
his trace `fails/26.11.10.2`, run 3, AMD Raphael iGPU, 16 cores) and his request of 11.11 (`kkt-26.11.11`
L4: "я не против чтобы оно наперед считало в конце концов.. и чтобы и процессор тоже использовался, для
тех же вычислений, почему и нет.."). Diagnosed from the trace (found-defects):

- 480: a Datamosh walk spends 96–98 % of its wall in `Ring::admit`: 106 ms a position playing, 16 ms of it on
  the device. The bound (8.33 ms at 60 fps) holds one piece; each next piece waits for the previous one's
  completion, which stands behind the display's composite (32.5 ms of ISP+VHS a frame, never admitted, so
  the ring cannot count it). Processor 20 % of one core of 16.
- 481: a pass never measured claims the whole bound: the first walk is one dispatch per piece (267 in 267).
- 482: the history advances only twice the display's reach a pass: with a loop, ~100 positions a lap of 715,
  six laps to finish it ("с 10-го прогона … наконец-то перестал кэтчить").
- 485: no rule says work a person waits for uses the machine.
- 486: `gpu_start` records are wrong under concurrent jobs (one query set per device), a batched walk's
  pieces have no start of their own, `Edit.a` is always 0, and no record says which executor ran a piece.

The class: **work a person waits for runs serially while the device or the cores stand idle** — a waiter
admitted only after the earlier piece is DONE (not when the device has room), a prediction that does not
exist yet, a look-ahead bounded by the display instead of by memory, a conversion on the decoder's one
thread, and records that cannot show any of it.

## What Changes

- **The ring admits by its predicted drain, not by completions** (480): every flight carries its submit
  time and predicted device time, the display's own present included (counted, never waiting); a lower
  class is admitted while the work it would leave in front of the display's NEXT submission stays within
  the bound, so it keeps pieces queued (submit-ahead) and the device never idles between them; a lower
  class's piece on a displayed ring is cut at half the bound so two fit. The display still waits at most
  the bound (t2253, the latency bench).
- **Cold passes are predicted** (481): the pass and dispatch times measured on this device are kept on
  disk (device + label + work size) and seed a new process; a compute job never measured learns from its
  own first pieces while it runs.
- **The history is computed ahead** (482 + L4): with a loop the prefetch walks to the loop's end (and
  round it) within the memo, without one well ahead of the playhead within the memo's budget, also on a
  pause inside a loop; the part past the display's reach runs at the lowest class, stoppable.
- **The processor as a second executor where the bytes allow** (L4): the preview's decoders convert to
  RGBA on several threads, as a render does (bit-exact); independent events' histories walk concurrently.
  A CPU twin of a float kernel (the codec's DCT, Camera ISP, VHS) is rejected: its bytes would differ from
  the device's and the choice would follow the load (invariant 11) — each decision in design.md.
- **The rule** (485): a new requirement — work a person waits for uses the idle device and idle cores within
  the owner's limits (rest costs nothing; the display first), measured from pass timestamps and thread time.
- **Records** (486): a query set per job in flight, `gpu_start` per piece with its own submit, the executor
  of each piece, `Edit.a` = the model's revision; `kt-cli trace gpu` reports a walk's admission share and the
  device's idle gaps.

## Capabilities

- video-plan (the ring, the pacing, the rule)
- telemetry (the records)
- media-index (the preview's conversion threads)

## Impact

`kt-gpu` (arbiter.rs, compute.rs, pass_time.rs, engine.rs present, a seeds module), `kt-engine` (player.rs
prefetch advance, video.rs stage walks), `kt-app` (decoder settings, Edit), `kt-cli` (trace gpu), kt-time
(trace events). Found-defects rows 480–482, 485, 486 close; new rows 490–509.
