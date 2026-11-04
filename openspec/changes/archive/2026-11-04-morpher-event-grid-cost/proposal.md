# Proposal

## Why

The owner (ktprompts v26.10.18, item 8, recorded in `docs/user-feedback.md`, round of
2026-10-27): an event grid in the morpher, with its automation going 0 → 12×30 → 12×10 → 0, makes fps drop.
"It is basically a simple pixel shader — so do we have extra computations? Answer briefly and
to the point why."

The code answers part of this. The grid IS one pass: one fused geometry pass per frame,
with no layer, decode, cache lookup or pipeline per cell, and no plan rebuild while the
automation moves. Two kinds of extra work exist all the same. First, once a cell is 4× or
more smaller than the frame, every output pixel averages 4×4 = 16 sub-samples of the source
(the item-309 anti-aliasing). At 12×30 the 360 cells are the same tile, and that tile is
computed 360 times. Second, the morpher's strength slot snapshots and mixes the frame, and
at strength 0 it still computes the whole grid and then throws it away. Which of these costs the owner his
fps on his machine is not proven yet. There is no counter for compose time, and the perf
panel says only which backend is in use. This change first measures, then removes the extra work that
can be removed with no change to a single output byte. The short answer for the owner is in
design.md.

## What Changes

- **Measure first.** Per-frame video cost counters in the plan executor (passes, sub-samples,
  snapshots and mixes, compose milliseconds), shared by the CPU and the GPU path.
  `kt-cli probe … --time` gains a video section (frames, compose ms p50/p95/max, backend,
  sub-samples and passes per frame). The performance panel shows the compose time and the
  backend that is actually in use.
- **Compute one cell, copy it.** When the event grid is the last geometry step of a fused
  series, every cell holds the same tile. Both executors compute that one tile with the
  existing supersampling and replicate it, plus the cells at the grid's outer edge, where
  sub-samples fall outside the grid. The output is byte-for-byte the frame of the full pass, and
  a test sweep proves that. When anything that differs per cell follows the grid (a geo step or a point-wise
  tap after it), the full pass stays.
- **A strength slot at 0 costs nothing.** When the layer's strength is exactly 0 on a frame,
  the executor skips the layer's slice and returns the dry snapshot. The result is the
  same bytes as today, without computing the grid.
- No new switch or quality mode, and no change to the supersampling rule (`ss_taps`, cap 4×4).
  The owner's "no switches" decision for the geometry series stands.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: a geometry series that ends in an event grid computes one tile and replicates it. The rule
  is exact, and the same on both backends (the `geom.rs` ↔ `geom_fused.wgsl` pair).
- `video-plan`: a strength slot whose strength is 0 skips the layer's slice. The frame cost
  is counted in the plan executor.
- `morpher`: the owner's case (a morpher event grid with automated counts) has a cost bound
  per frame, with no renderer rebuild while the automation moves.
- `ui-timeline`: the performance panel and `kt-cli probe --time` tell the video compose cost,
  not only the audio one.

## Impact

- Code: `src/libs/kt-fx/src/geom.rs` (tile detection in the program builder, `exec_geom_cpu`
  tile path, a pure `sample_cost`), `src/libs/kt-fx/src/plan.rs` (no change to segmentation),
  `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` + `src/libs/kt-gpu/src/engine.rs` (a tile
  pass into a small target plus a replicate pass), `src/libs/kt-engine/src/video.rs` (the strength-0 skip at
  ~2469-2498, the cost counters), `src/libs/kt-gpu/src/engine.rs` (the strength-0 skip at the
  Snapshot/Mix ops ~347-356), `src/apps/kt-cli/src/main.rs` (probe `--time` video section),
  and `src/apps/kt-app/src/app.rs` (perf panel line).
- Goldens: none move. Every speed-up must be byte-exact against the current CPU reference
  (t943, t945). If the sweep finds a case that is not, that case keeps the full pass (design D2).
- Invariants: 7 and 11 are held by construction and checked by the tests above. 12 is untouched: the counters
  are plain integers in the frame path, with no allocation.
- No project-format change and no new dependency.
