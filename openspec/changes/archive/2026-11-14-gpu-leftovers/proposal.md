# Proposal: gpu-leftovers

## Why

Eight rows of `docs/found-defects.md` left by gpu-throughput, catch-up-shows-loading and class-walk-revision,
all diagnosed, none fixed (the owner, 2026-11-10: fix diagnosed defects without asking):

- 491: a display that saturates the device (his iGPU: 32.5 ms of Camera ISP + VHS a frame at a 16.7 ms
  period) still leaves a lower class one bound in front of every display frame, so the display falls
  further behind while gaining nothing.
- 510: a point-wise run is run fused or as its parts as a timing trial decides; on a card the two forms are
  bounded (≤ 1 LSB a boundary), not equal, and no key names the form: the readback memo, the kept
  composites and the export keep whichever ran — invariant 11 rests on a compiler.
- 511: the compute host's clock calibration submits outside the ring.
- 520: the compute host takes a uniform buffer, writes it and creates a bind group for every dispatch
  (~2.4 ms of a 267-dispatch history walk).
- 521: the readback memo's LRU finds a key by a linear scan, per frame.
- 522: an export writes a frame that came out incomplete for a reason other than its source (a blend pair
  whose composite reported incomplete while every frame is cached; catch-up-shows-loading D5 claimed "the
  export retries", and it did not).
- 523: the readback memo keeps incomplete composites, pushing complete ones out of its 64 MB.
- 524: reel strip rows written with empty generator cells (before t2455's fix) stay on disk until the
  reel's revision moves.

## What Changes

- **The arbiter gives a lower class the display's measured slack** (491): the display's present cost is
  remembered from its own presents; in front of the display's next present a lower class is admitted only
  into the period the display leaves free, at most the bound; a display that fills its period leaves the
  `near` class (what feeds the display) a quarter bound and the classes below it nothing until it stops.
- **The form of a run is named** (510): a frame's output form is a pure function of its shapes' states —
  the own passes until a shape's verdict, then the verdict; the trial times the fused pass as a shadow pass
  beside the own output instead of alternating outputs. A process-wide forms revision moves whenever any
  shape's output form changes and is part of the readback memo's and the present memo's keys. An export
  composes runs in the SETTLED form: fused wherever the shape compiles, compiled in the frame when not
  warm, no trial — a function of the plan and the device only.
- **The compute host admits its calibration on the ring** (511) and its warm-up dispatch.
- **The compute host writes one uniform arena per job with dynamic offsets and reuses a bind group** for
  the same shader and buffers within the job (520).
- **The readback memo is an LRU by a generation counter** (521) **and keeps only complete composites** (523).
- **An export never writes a frame incomplete for its own reasons** (522): before a frame that came out
  incomplete is written, the export asks the cache for what it lacks and decodes each missing frame alone
  (serial and pipelined alike), then composes the frame again — twice at most. Only what the input itself
  cannot give is written without its layer and named, as before (t1602, t1606: the owner's contract that a
  damaged file costs its frames, not the render); the same answer twice from the same cache is the input's.
- **The reel strip's disk key carries a version** (524): rows written before it are misses and are rendered
  again once.

## Capabilities

### Modified Capabilities
- `video-plan`: the run form rule, the memo keys, the ring's room by display cost, the compute host's
  admission and binding.
- `export`: an incomplete composite with every frame cached is composed again or fails.
- `compositions`: the strip row key's version.

## Impact

`kt-gpu` (`arbiter.rs`, `compute.rs`, `compiled.rs`, `engine.rs`, `pool.rs`, `runs.rs`), `kt-engine`
(`export.rs`, `export_pipeline.rs`), `kt-app` (`reel_preview.rs`). Tests t2510–t2520. No format change.
