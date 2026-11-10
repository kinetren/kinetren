# Design: stream-history-on-device

## Context

His trace (`fails/26.11.10/trace`, release 87f6f38d, kkt-26.11.10 J2/J3/J7) on
`problem_region_26_11_09`: the prefetch thread (class near) advanced the Datamosh history one position
per call; a position was two compute jobs (`codec::encode_rgba`, `codec::decode`), each
`ComputeHost::run` = upload, submit, `piece.end`, blocking `poll(wait)`, map and copy back. A job of
0.2 ms of device time took 46 ms of wall time while playing (114 ms a position, 7.5 positions/s), ~18/s
on a pause. The arbiter's `Piece::cut` waited for each piece of a lower-class job before admitting the
next (127 `gpu_piece` records, ~3.1 s of a 7.2 s walk). The memo was sized by the loop, so the walk from
the chain's anchor to the loop (with mosh on, every I-frame is dropped and the chain runs back to the
stream start or a reset) was evicted and walked again: 1413 positions computed for 618 distinct.

Baseline here (llvmpipe, release, `kt-cli stream-cost` 4419.91–4422.00 s, 2 laps, --loop): lap 1 126
frames in 7.2 s, 57.2 ms a frame, 297 jobs (2.36 a frame), 10.97 MB uploaded and 6.69 MB read back a
frame; `kt-cli preview` 4425–4427 s 640×360 loop 60 s: V-fps 38.8, 239 compute jobs at 13.85 ms each.

## Goals / Non-Goals

**Goals:** no per-position device round trip in a history walk; the bytes unchanged; the arbiter never
waits for a lower-class piece while the bound has room; V-fps counts new pictures; a re-walk bounded by
a checkpoint stride; a trace that tells a late start from a late signal.

**Non-Goals:** shortening a first visit's walk (the chain is the look, the owner's decision); the audio
half's codec (CPU, no device job, see the class walk); asynchronous jobs across walks (D2).

## Decisions

### D1. A walk is a recorded program, flushed as one job per batch
`walk.rs` replays the per-position algorithm of `mosh::broken`/`clean_at` symbolically: the codec's
operations (convert, search ×2, residual ×3, reconstruct ×3; vector edit, residual drop, decode ×3) are
recorded as steps of ONE `compute::Job` over buffers the walk allocates; a value produced earlier in the
batch is a buffer index, a value from the memo is an upload. The existing `ComputeHost::run` already
runs any number of steps in order with the buffers on the device: no host API change. Working buffers
(planes, vectors, residual) are recycled within a batch once their last reader is recorded — wgpu orders
storage hazards between dispatches — but never an uploaded buffer (`write_buffer` lands before the whole
submission). A batch closes at `MAX_BATCH` positions or `BATCH_UPLOAD` bytes of uploads (the source
RGBA dominates: 3.7 MB a position at 720 lines), at a missing frame and at a stop; its reads are the kept
pictures and the continuation state.

Rejected: resident buffers across jobs (a handle API in `kt-plugin-api::compute`): every batch ends with
one wait anyway, and the continuation state is 1.4 MB a plane set; the round trip per batch, not the
bytes, is the cost. Rejected: a CPU-side vector edit between device steps: it needs the vectors back,
one round trip a position again.

### D2. One wait a batch, not none
The walk thread still waits for each batch's buffers. With K positions a batch the round trip is paid
once per K. Overlapping the next batch's recording with the device would need the batch's end state
before its readback (resident handles, D1 rejected) for little: recording a batch is a few memcpys.

### D3. The vector edit is exact on the device
`transform` rounds `scale·(x·cos − y·sin)` in f64; WGSL has no portable f64. The search clamps vectors
to ±search ≤ 64, the second input's too, so the CPU computes the edit for all 129² inputs once per
(scale, rotate, swap) and the device looks it up (133 KB, kept in the walk by the rows' bits). Loss is
`lowbias32` integer hashing, ported to WGSL; the threshold `h < loss·2³²` is `h < ceil(loss·2³²)` in
integers (2³² — every block). The residual of a lost block is zeroed by a drop step into a copy (the
packet's own residual is read by the bloom repeat and must stay intact).

### D4. What comes back
The host tells the memo its view (`StreamMemo::set_view`, a tick; `StreamLayout::shown_from` gives the
position): the loop region's start while the playhead is inside it, else the frame on screen. A walk
reads back the frame asked, the positions at or after the view, the checkpoints, and the clean
encoder's last state per side (as a recon-only entry, so the next walk continues there). An export never
sets a view: every position comes back, as before. A bloom source (a position that a later position's
packet repeats) keeps its whole clean packet, or the run would re-encode it from the GOP start every
batch.

### D5. Checkpoints in a pinned tier
Every `CHECKPOINT` (16) positions, counted from position 0 (a grid stable across walks), the broken
picture and the clean recon per side go into the memo's pinned tier: `put_pinned(key, v, bytes, rank)`,
rank = trailing zeros of `k / 16`. Pinned entries are skipped by the LRU; the tier holds at most half the
memo's bound and drops the lowest rank first, oldest among equals — so it thins to every 32nd, 64th…
position and the re-walk bound doubles instead of a hole opening. `needs` and `clean_at` accept the
recon-only entry for a position before the one asked (never for the position itself, whose packet is
needed). Sizing (video-plan): the want adds two pictures a checkpoint of the walk from the chain's
anchor (`StreamKernel::walk_from`) to the region's end.

### D6. Arbiter: submit and go on
`Piece::cut` submits the piece (its completion frees the ring by the work-done callback, as `end`
does) and admits the next piece with a claim of the next pass's prediction instead of the whole bound,
so the admission waits only while the ring's bound is full or a higher class waits. On a ring with a
display `cut_before` still ends a piece at the bound, so no multi-pass over-bound piece stands in front of
the display (t2253).

### D7. GPU start on the CPU clock
The compute host writes a timestamp in an empty submission at its first job and when its calibration is
older than 10 s, brackets it by `trace::now_ns()` before the submit and after the wait, and keeps the
narrowest of the last few brackets: `cpu = (gpu − g0)·period + mid`, error = half the bracket. Each job
records `GpuStart` (93): submitted, device start, device end, error; a device without `TIMESTAMP_QUERY`
records `aux = u32::MAX`. Lavapipe's clock is CLOCK_MONOTONIC already; the calibration does not assume
it.

### D8. V-fps and the marker
`Composed.held` carries the composite's stream-held flag (the player already knows it). The app counts
a new picture only for a non-held frame; held frames go to `perf.video_frames_held`. The marker is a
small catalog string in the preview's top-left corner over the picture (a `painter.text` on a faint
backing), drawn while `preview_held` is set — no repaint of its own: it changes only when a composite
lands. The owner's Loading decisions stay: the scrim is for an incomplete frame after 180 ms.

### D9. Found while measuring
- The compute host timed only the first 32 dispatches of a job (`STAMPS` 64): a batched walk has
  hundreds, so its device time was undercounted and its later dispatches were never predicted (the
  arbiter could not cut there). `STAMPS` is 2048.
- The prefetch's run walked the frame on screen in the same batch as the frames ahead: incomplete
  composites rose 1 → 51 in a minute's loop. `stream_advance_run` walks to the frame on screen first,
  then to the run's end; a walk also closes its batch at the view (`shown_from`). Incomplete went to 0.
- A checkpoint is pinned even where it is shown (the LRU may drop what is shown): t2353 caught it.

## Measured (release, llvmpipe — no card on this machine)

| What | Before | After | How |
|---|---|---|---|
| problem region 4419.91–4422 s, per-frame advance | 297 jobs, 57.2 ms a frame, 20.4 positions/s, 11.0 MB up / 6.7 MB back a frame | 126 jobs, 26.6 ms a frame, 43.9 positions/s, 7.3 / 3.0 MB | `kt-cli stream-cost h.kt 4419.91 4422 --laps 2 --loop` on his relinked copy |
| a 191-position walk (showcase mosh cut, 70.0–70.4 s) | 391 jobs, 3.9 s, 49 positions/s, 26 MB up / 16 MB back a frame | 14 jobs (13.6 positions a job), 3.6 s, 53 positions/s, 11 / 4.3 MB | `kt-cli stream-cost m.kt 70.0 70.4 --laps 2 --drop-lru` |
| device share of that walk | 1.26 s of 3.9 s | 1.34 s of 3.6 s (2.5× wall over timed dispatches: lavapipe's per-pass barriers, row 400) | `kt-cli trace … gpu` |
| re-walk after the LRU lost the history | the whole walk (183 positions) | 6 positions, 1 job, 0.06 s | lap 2 of `--drop-lru` |
| preview loop 4425–4427 s at 640×360, 60 s | V-fps 38.8, 239 compute jobs, incomplete 1 | V-fps 37.9, 17 jobs, incomplete 0 | `kt-cli preview h.kt 4425 4427 --size 640x360 --loop 60` (the loop is VHS + Camera ISP bound) |
| compute start delay | not recordable | near p50 7.5 ms, run p50 299 ms, buffers after end p50 5.9 ms | `kt-cli trace … gpu` |

On a card the device work of a position is ~0.2 ms against a 46 ms round trip in his trace, so the
walk was ~99 % waiting: one wait per batch of up to 32 positions should bring it near the device's own
speed — unmeasured here (row 406); his next trace's `trace gpu` lines say it.

## The class walk

Mechanism: a dependent sequence of device work waits for the device per step, and its intermediate
state crosses the bus per step. Checked:

| Place | Waits per | Found |
|---|---|---|
| Datamosh history (`mosh::broken`, `clean_at`) | position, two jobs | fixed (D1) |
| Datamosh region blend (`region_blend`) | frame, CPU | one readback of the clean picture per frame asked; stays |
| Datamosh's sound (`sound.rs`, `audio_codec`) | none: CPU codec on the audio render thread | no device job; nothing shared with the picture's cost |
| Arbiter `Piece::cut` | piece | fixed (D6) |
| Arbiter `Ring::admit` completing the oldest piece | bound full | intended: that is the bound |
| Compute host `run` | job: map wait per read buffer | one `poll(wait)` then maps; the per-buffer `poll` after the first finds them ready — measured cost nil; stays |
| Compute host timestamp readback | job: a second map | ready at the next poll (same submission); stays |
| `generator::shader_frame` (compute generators) | frame | one job per frame, independent frames, no state between: not the class |
| Camera ISP, VHS Tape (render passes, `engine.rs`) | frame | one submission per frame through `Piece`; fixed by D6 where cut |
| VHS sound, Camera mic | none: CPU on the audio thread | not the class |
| Export pre-roll (`stream_preroll`) | batch of decodes then `stream_advance` | gets D1 through the kernel; every position kept (no view) |
| `kt-gpu` `wait_presented` | frame | measured waits, t2260; not the class |
| Readback ring of the export (`submit`/`collect`) | frame, ring of 3 | pipelined already |
| Offline engine (reel strip, poster) | frame | independent frames; not the class |
| Pass timer `harvest` | never waits | not the class |

Left: rows 400+ in `docs/found-defects.md`.

## Risks / Trade-offs

- A batch of K positions holds K source frames' RGBA as uploads: bounded by `BATCH_UPLOAD`.
- Fewer pictures kept before the view: a held frame from the history before the loop is no longer in the
  memo — the marker and Loading say what the preview is doing instead.
- Pinned tier shares the memo's bound: a loop that needs the whole bound loses checkpoints first (they
  thin, never vanish while one fits).
