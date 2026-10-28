# Design

## Why the grid costs fps (short answer for the owner)

**Yes, there is extra computation, but it is not "per cell".** The grid is one pass per
frame: there is no layer, decode, cache lookup, pipeline or plan rebuild per cell, and the
automation does not rebuild anything. The extra work is of three kinds:

1. **Anti-aliasing multiplies the pass by 16, and every cell repeats it.** When a cell is
   4× or more smaller than the frame, every output pixel averages 4×4 = 16 sub-samples of the
   source, each a bilinear read (item 309, put in so the grid does not shimmer). At 12×30 on a
   960×540 preview that is 8.3 M sub-samples per frame. Yet all 360 cells are the same 80×18
   tile, so 1/360 of that work would give the same bytes. The "simple pixel shader" is
   16 times heavier than it looks, and 359/360 of it is repetition.
2. **The morpher's strength slot computes the grid even where it is invisible.** Where
   the block's fades or gates put the strength at 0, the grid is still computed in full and then mixed away.
   Wherever the strength is below 1, the CPU path also copies the whole frame once as a snapshot.
3. **If the preview is on the CPU, all of this is scalar f64 on the UI thread.** That happens
   on a machine whose only adapter is the software D3D12 one, which is refused, as it is under wine
   without a real GPU. It also happens after a GPU loss. Then 8.3 M sub-samples cost on the order of
   100–250 ms a frame (an estimate, not yet measured), which means single-digit fps.

Which of the three costs **you** the fps is measured first (stage 1). Look at the
Performance panel line "FX: GPU (…)" / CPU on the machine where it drops. The fix is (1) one
tile computed and copied, (2) strength 0 skips the layer. Both give identical bytes, so there is
no switch and no golden moves.

## Context

All facts below were read in the code on 2026-10-28 and are marked as observed. Hypotheses
are marked as such.

**Observed: what the grid is.** `FxInstance::EventGrid { cols, rows, center }`
(`src/libs/kt-model/src/project.rs:6013`), params `Count`/`Smooth` in 1..512
(`project.rs:7046`; an automated 0 clamps to 1). A morpher adds it by appending its patch chain
and a `LayerMix` strength slot to the target clip (`MorpherEvent::apply_to`,
`project.rs:2526-2555`). The fused geometry builder makes it one `Step::Grid`
(`src/libs/kt-fx/src/geom.rs:882-905`) → `GeomPrim::Fold`. On the GPU it is the Fold arm of
`geo_map` (`src/libs/kt-gpu/src/shaders/geom_fused.wgsl:73-86`) in one `FxOp::GeomFused`
pass (`src/libs/kt-gpu/src/engine.rs:331`).

**Observed: nothing is rebuilt per frame.** Morpher expansion (`morph_pass`, `project.rs:8622`)
runs when the renderer is built (`VideoRenderer::build_reel` → `resolve_reel`,
`src/libs/kt-engine/src/video.rs:838`), and the renderer is rebuilt only on a project swap
(`src/libs/kt-engine/src/player.rs:346-359`, `:1432`, keyed by `project_version`). Automation
is compiled once per build (`AutoPlan::compile`, `video.rs:928`) and applied per frame by
index (`video.rs:1617-1626`). GPU pipelines are created once (`engine.rs:869, 923`). The uniform
arena grows only (`:1036-1045`), and `cols`/`rows` reach only the uniform.

**Observed: the multiplier.** For an affine program the sub-tap count is computed once from
the Jacobian: Fold contributes `in_w / cw` and `in_h / ch` (`geom.rs:1225-1232`), and
`ss_taps` caps it at `SS_MAX = 4` per axis (`geom.rs:562-586`). So a grid with ≥4 columns
and ≥4 rows takes 16 sub-samples per pixel, and 2×2 takes 4. The CPU executor
(`exec_geom_cpu`, `geom.rs:1921-1955`) is a scalar f64 double loop, single-threaded
(`src/libs/kt-fx/CLAUDE.md`: "The merged pass is SCALAR f64"). The WGSL fragment does the same loop
(`geom_fused.wgsl:505-528`), up to 64 `textureLoad` per pixel. EventGrid is never an identity,
even at 1×1 (`geom.rs:88`).

**Observed: the cells are one tile.** The Fold arm maps a coordinate to
`(x − floor(x / cw)·cw) / cw · in_w − 0.5` with integer `cw`, `ch`, `offx`, `offy` (`geom.rs:884-893`).
Inside the grid every cell's pixel `(i, j)` therefore has the same sub-sample coordinates
relative to its cell. If no primitive acts AFTER the Fold, the inverse map is the same and
every interior cell holds the same bytes. The exceptions are the pixels whose sub-samples
cross the grid's outer edge (`x < 0 || x >= gw` → dead) — the outer ring. **Assumption,
proven by t943 before any executor relies on it:** f64 rounding of `x − kx·cw` gives the same
value in every cell also for 3 taps per axis (non-dyadic offsets). The design falls back if it does not
(D2).

**Observed: strength 0 still computes.** The CPU slices (`video.rs:2455-2505`) snapshot the
frame at the slot's `from` (`rgba.clone()`), run the slice, then `blend_frames_cpu(dry, frame, k)` for
`k < 4096`, including `k = 0`. The GPU emits Snapshot plus Mix passes when `k < 4096`
(`engine.rs:347-356, 441-447`).

**Observed: the preview path.** The preview composes on the UI thread
(`src/apps/kt-app/src/app.rs:6189` → `Player::compose_preview_into`, `player.rs:840-881`).
The GPU path reads back synchronously (`device.poll(wait_indefinitely)`, `engine.rs:1095-1118`)
and the frame is uploaded to egui. The CPU fallback is taken on `TooLarge` for one plan, or for the
whole session after DeviceLost/Validation (`player.rs:862-879`). The software D3D12 adapter (WARP)
is refused at start (`engine.rs:817-828`), so a Windows machine without a real GPU — wine
included — renders FX on the CPU. llvmpipe is accepted as an adapter (`engine.rs:809-810`), and its
"GPU" is the CPU.

**Observed: no number exists.** The perf panel shows FPS, V-fps, skips, the preview size and
"FX: GPU (<adapter>)" / CPU (`app.rs:6928-6937`, `i18n/en.toml` `[perf]`). There is no compose
time, pass or sub-sample count. `kt-cli probe --time` measures only the audio render
(`src/apps/kt-cli/src/main.rs:465-540`). No test covers the morpher's or the grid's video cost.
t673/t676 guard only the zero-cost-without-use path.

## Goals / Non-Goals

**Goals:**
- Turn "drops fps" into numbers: per-frame passes, sub-samples, snapshots/mixes and
  compose ms, in a test, in `probe --time` and in the panel.
- Remove the repetition (one tile instead of N×M) and the invisible work (strength 0),
  with byte-identical output on the CPU reference and the GPU within its existing class.

**Non-Goals:**
- Changing the anti-aliasing rule (`ss_taps`, cap 4×4) or the grid's look. At 30× minification 4×4 still
  aliases. A pre-shrunk (mip) source would fix both quality and cost, but it changes bytes:
  see Open Questions 2.
- Moving the preview compose off the UI thread, or an asynchronous GPU readback. These are
  architecture changes of `transport`/`video-plan` with their own invariants (6, 12). If
  stage 1 shows the readback dominates on the GPU, they get a change of their own.
- A SIMD CPU geometry pass (the AVX2 tier exists for other kernels). It is not needed once
  the tile rule cuts the work by more than 10×.

## Decisions

### D1. Measure through the plan executor, not by timers in the UI
A `FrameCost { passes, subsamples, snapshots, mixes, compose_ns }` is filled by the plan
executor that both backends run (the CPU slices in `video.rs`, the GPU op list in
`engine.rs`). `subsamples` comes from a pure `GeomProgram::sample_cost()`, so the count is
deterministic and testable, unlike time. The panel and `probe --time` read the same struct.
Alternative: a profiler span around `compose_preview_into`. It gives time only, and it cannot
say whether the cost is passes or sub-samples, or which backend paid for it.

### D2. One tile, copied, only where it is provably exact
The program builder marks a fused series as `tile: Some(TileSpec { cw, ch, offx, offy, cols, rows })`
when the Fold is the last primitive and no tap op's stage comes after it. Executors:
- CPU: compute one interior tile (`cw×ch`, sub-sampled as today) into scratch, copy it
  row by row into each cell, compute the outer ring of the grid pixel by pixel through the
  full path, and fill the field outside the grid as today.
- GPU: a tile pass into a pooled `cw×ch` target, then a replicate pass that reads it with
  integer `textureLoad` (no filtering, so it is exact). The outer ring is drawn by the full
  fused shader with a scissor or a ring mask. Edited as a pair with `geom.rs` in one commit.
The ring width is what t943 proves: one pixel when the sub-sample offsets stay within
±0.5 px. If the sweep shows any interior cell differing from the full pass (the 3-tap f64
case), the builder does not mark the tile for that tap count, and the full pass stays there. The
bytes do not move, and the owner's case (4×4 taps) is still covered.
Alternatives: (a) a mip or pre-shrunk source sampled once per pixel — cheaper still and
better looking, but every grid golden changes (Open Questions 2); (b) a "fast grid" switch
— rejected by the owner's R.44 rule "no switches"; (c) caching the grid output across frames
— it changes every frame during playback (the source moves), so it only helps a still frame.

### D3. Strength exactly 0 skips the slice; the test, not the comment, guards the bytes
In the CPU slices, when the slot's evaluated `k == 0`, the slice from `from` to the slot is
not executed and the snapshot becomes the frame. The GPU op list drops the slice's ops and
the Mix, and keeps the snapshot as the output. t945 checks that this equals `blend(dry, wet, 0)`
of today byte for byte. Only an exact 0 skips: a strength of 0.001 is a visible mix.

### D4. Owner's case as a fixture built by the test
t946 builds the project in code (synthetic 960×540 source, morpher block, automation keys
1→12×30→12×10→1, fades). This follows "tests must not write into the tree" and R.67's "our
fixtures, not the owner's projects". The before-change hash of the rendered range is
recorded in stage 1, before the executors change, and asserted after.

## Risks / Trade-offs

- [The tile rule's exactness may fail for some tap count or frame size] → t943's sweep is
  the gate. A failing combination keeps the full pass (D2), and nothing moves.
- [The GPU gets two more passes and a pooled target] → Each is far below the 16-tap full pass
  at ≥4 cells per axis. For 1×1 and 2×2 the builder does not mark a tile (no gain), and t944
  checks the pass count.
- [Stage 1 may show that the owner's drop is the GPU readback, not the grid] → Then
  this change still removes 16× and invisible work, and the measurement names the next
  change (Non-Goals). The short answer above says so and does not promise a cure.
- [Counters in the frame path] → Plain integers in a struct the caller owns. No allocation or lock
  (invariant 12), and the audio thread is not touched (invariant 2).

## Migration Plan

No data migration. Each stage is byte-identical, so rollback is a revert.

## Open Questions

1. **Which backend does the owner's machine show when the fps drops?** (the panel line
   "FX: GPU (…)" / CPU, and the adapter name). Recommended: ask with the stage 1 build. If it
   says CPU, or the adapter is WARP/llvmpipe, then kind 3 is the main cost, and D2 is the cure.
2. **Should the grid's look improve too (a pre-shrunk source, no aliasing at 30×)?** It is
   cheaper still and cleaner, but it regenerates the grid goldens (t26 group) and changes how
   existing projects look. Recommended: not in this change. Offer it to the owner with
   before/after shots once stage 3 is in.
