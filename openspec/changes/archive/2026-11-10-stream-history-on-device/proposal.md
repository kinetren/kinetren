# Proposal: stream-history-on-device

## Why

The owner's run of stage 9 (release 87f6f38d, `kkt-26.11.10` J2, J3, J7; his trace
`fails/26.11.10/trace`): on `problem_region_26_11_09` (Datamosh at 720 lines) the preview "hangs
without Loading, little processor load, moves only after many laps; V-fps 20 where at most 5 are new;
on a pause Loading and then it computes, as if with sleeps"; after a jump forward the coverage strip
creeps from the chain's anchor to the playhead. Diagnosed from his trace (found-defects):

- 391: one thread advances the Datamosh history position by position as two compute jobs, each a
  submit, a blocking wait and a readback: 46 ms round trip for a 0.2 ms job, 7.5 positions/s while
  playing, the device busy 12 % of the walk;
- 392: the GPU arbiter waits for each lower-class piece to finish before admitting the next
  (`Piece::cut` → `wait`), ~45 % of a 720-line walk;
- 393: V-fps counts tick changes, so a held stream frame shown again counts as a new picture;
- 394: the stream memo is sized by the loop, not by the walk from the chain's anchor to it, so the walk
  is evicted and recomputed (1413 positions computed, 618 distinct, 15.6 GB of evictions over the run);
  no sparse tier bounds a re-walk;
- 395 (its re-walk part): after an eviction or a jump the walk starts again at the anchor;
- 398: the trace cannot tell a late GPU start from a late completion signal.

The class is "a dependent sequence of device work waits for the device per step, and its
intermediate state crosses the bus per step". The change walks every compute and stream stage and every
GPU job that waits per job or per piece.

## What Changes

- **The Datamosh history runs on the device in batches**: a walk encodes up to K positions in ONE
  submission — the clean encoder's planes, the broken decoder's reference, the vector edits (scale,
  rotate, swap through an exact table; loss by the integer hash) and the residual drop are device steps
  whose dependencies stay on the device. Only the pictures that are shown or kept come back: the frame
  asked, positions from the host's view on (the loop region or the playhead), and sparse checkpoints.
  Same bytes as the per-position walk (invariant 11; t1239, t2270–t2278 untouched).
- **The prefetch advances a run of frames ahead as one walk** instead of one call per frame.
- **The arbiter admits the next piece of a lower-class frame or job against the device time in
  flight**: a cut submits the piece and goes on; it waits only when the ring's bound is full. A ring
  with a display still never holds an over-bound piece in front of it (t2253).
- **V-fps counts new pictures of the moment asked**: a held stream frame is counted apart ("held"),
  never as a new picture; every frame held → V-fps 0.
- **The stream memo covers the walk**: its want adds a sparse checkpoint tier (the decoder's reference
  and the clean encoder's state every N positions, pinned outside the LRU, thinned evenly when the
  ledger's grant shrinks), so a re-walk after an eviction or a jump is bounded by N positions. The first
  visit still integrates every P-frame since the anchor: the chain length is the person's look decision.
- **The trace records when a compute submission starts and ends on the device on the CPU clock**
  (calibrated against the device's timestamps; the record says when the device gives none).
- **A playing preview that shows a held stream frame says it is catching up**: a light marker over the
  picture, not the Loading scrim.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the Datamosh history's batched walk on the device, its readback rule and checkpoints.
- `video-plan`: arbiter pieces admitted against the time in flight; the stream memo's walk-and-checkpoint
  sizing; the prefetch's advance of a run of frames; the held frame's catching-up state.
- `telemetry`: the calibrated GPU start/end record, the walk record; V-fps counts new pictures, held
  frames counted apart.
- `ui-timeline`: the catching-up marker over the preview.

## Impact

- `src/plugins/fx-video-codec` (codec.rs, mosh.rs, new `walk.rs`, two shaders), `kt-plugin-api`
  (`StreamLayout::shown_from`, `StreamKernel::walk_from`, `StreamMemo` pinned tier and view),
  `kt-gpu` (arbiter `Piece::cut`, compute host's start record and calibration), `kt-engine`
  (`video_stream.rs`, player's prefetch, `Composed.held`), `kt-app` (V-fps, held/s, marker, i18n),
  `kt-time` (trace events 96, 97), `kt-cli` (stream-cost prints positions/s and readbacks).
- No file format change; no OTIO export change.
