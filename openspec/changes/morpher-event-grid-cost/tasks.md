# Tasks

Test numbers t942–t947 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done. Speed is judged only on a
release build.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's item 8 of ktprompts v26.10.18 is in `docs/user-feedback.md` (round of 2026-10-27), the short answer is in design.md, and the change's artifacts are written. Verify `openspec validate morpher-event-grid-cost --strict` is green

## 1. Measure first (no behaviour change)

- [x] 1.1 `GeomProgram::sample_cost()` (pure) and `FrameCost` filled by the CPU slices (`video.rs`) and the GPU op list (`engine.rs`) (design D1). Write t942 with today's numbers as the "before" half (1×1, 2×2, 12×10, 12×30 at 960×540: 518 400 / 2 073 600 / 8 294 400 / 8 294 400). Its "after" half is RED. Verify the before half is green and record the RED line here
  - `kt_fx::video::FrameCost { passes, subsamples, subsamples_unknown, snapshots, mixes }` rides in
    `FxScratch::cost`: the CPU executor adds a pass per fused run (with `GeomProgram::sample_cost()`),
    legacy geometry step and barrier; `apply_chain_with_mixes` adds snapshots and mixes; the renderer
    resets it per compose and gives it as `VideoRenderer::last_cost()`. The GPU side sums its op list
    (`ChainSpec::cost`/`Job::cost`, nests included; a readback-memo hit costs zero) into
    `VideoGpuExec::last_cost()` (a defaulted trait method). `Player::video_cost()` = the backend that
    actually composed + its cost + compose ns (the preview path; the wall clock is legal there).
    `sample_cost` is `None` for a perspective program (taps per pixel) and sets `subsamples_unknown`.
  - t942 (`src/libs/kt-fx/tests/grid_cost.rs`): the design's numbers hold — 518 400 / 2 073 600 /
    8 294 400 / 8 294 400 for 1×1, 2×2, 12×10, 12×30 at 960×540, and the CPU executor reports one
    pass and exactly those sub-samples. Its "after" half joins with the tile rule (stage 3).
- [x] 1.2 Write t946 in a new `src/libs/kt-engine/tests/morph_grid_cost.rs` (the owner's case built in code, design D4). Record the range hash of the CURRENT code into the test before any executor changes. Verify the rebuild and hash assertions are green and the per-frame cost bound is RED
  - t946 (`src/libs/kt-engine/tests/morph_grid_cost.rs`): the owner's case in code (synthetic 960×540,
    a 6 s block, grid 1×1 → 12×30 → 12×10 → 1×1 at 0/2/4/6 s, fades 0.5 s). ONE renderer renders 12
    frames; every 4th equals a fresh renderer's (nothing rebuilt per frame); the range hash of the
    current code is pinned: `0xc73e46f0e915bf00`. The cost bound is its own test, t946b, `#[ignore]`
    with the reason "stage 3 turns it on" (so `cargo test` stays green on this commit). RED when
    run: "the peak frame at Tick(45000) reads 8294400 sub-samples, the full 16-tap pass is 8294400" —
    the peak is at 0.5 s, where the grid already has 4+ cells per axis, not at 12×30.
- [x] 1.3 `kt-cli probe --time`: the video section (frames, compose ms p50/p95/max, backend, passes and sub-samples per frame). Write the probe half of t947 in `src/apps/kt-cli/tests/cli.rs` (it checks fields and counts, not times). Verify it is green with `KT_VIDEO_GPU=0` and with the default
  - `kt-cli probe … --time` gains "-- video compose cost" (`probe_video_cost`): the range's frames
    are decoded into a cache FIRST, then each frame's compose is timed on the backend `render` would
    use (headless GPU by default, `KT_VIDEO_GPU=0` CPU): backend, frames (incomplete), compose ms
    p50/p95/max, passes/sub-samples (with the peak)/snapshots/mixes per frame.
    `compose_export_into` resets the counters per frame too. t947's probe half (`cli.rs`, its own copy
    of the owner's case; `KT_T947_SAVE=<path>` keeps the project) checks the fields and counts on the
    CPU and that the default backend counts the same peak. Green with both.
- [x] 1.4 Investigation on a RELEASE build (`cargo build --release -p kt-cli`): run the t946 project through `kt-cli probe … --time`, saved by the test helper into the scratch dir. Run it once on the GPU and once with `KT_VIDEO_GPU=0`, for grids 1×1, 2×2, 12×10 and 12×30. Record compose ms, passes and sub-samples here, and state which of the three kinds of the short answer dominates on each backend. Send the owner Open Question 1 with these numbers
  - Release `kt-cli probe … 0 2 --time` on the t947 project (saved with `KT_T947_SAVE`) and four
    static variants of it (grid fixed, no automation, no fades), 960×540, 60 frames each; the
    adapter of this machine is llvmpipe (a software rasterizer — its GPU times are pessimistic):

    | grid | sub-samples/frame | GPU (llvmpipe) p50 | CPU p50 |
    |---|---|---|---|
    | 1×1 | 518 400 | 3.2 ms | 14 ms |
    | 2×2 | 2 073 600 | 4.9 ms | 65 ms |
    | 12×10 | 8 294 400 | 12.1 ms | 274 ms |
    | 12×30 | 8 294 400 | 12.0 ms | 274 ms |
    | owner's case | 7 257 600 avg (max 8 294 400) | 11.8 ms | 277 ms |

    Kind 1 (the 16× anti-aliasing, repeated per cell) dominates on BOTH backends: ×3.8 on the GPU,
    ×19.5 on the CPU against 1×1, and 12×10 costs exactly what 12×30 does. Kind 2 (strength) is one
    snapshot per frame and a mix on the fade frames (0.2 per frame) — small. Kind 3: on the CPU
    fallback the grid is 274 ms a frame, under 4 fps. The first GPU frame (8.6 s) is pipeline
    creation, not the grid. The GPU counts a Mix as 2 passes (`FxOp::passes`), the CPU as none — the
    sub-samples, which matter, agree. Open Question 1 was answered by the owner before this
    ("most likely it was on the GPU"); the table goes to him with the short answer.
- [x] 1.5 Commit stage 1
  - Workspace green except t411 and t640 (both on the load-flake list; green alone); clippy: no new
    warnings (the collapsible `if` in `kt-cli` main.rs pre-exists, moved by the insertion).

## 2. Strength 0 skips the layer

- [x] 2.1 CPU slices and the GPU op list skip the slice when the slot's `k == 0` (design D3). Write t945 in `src/libs/kt-engine/tests/morph_slice_render.rs` on the CPU and the GPU. Verify t945 is green, t267/t267b/t268/t181 are green, and t946's hash is unchanged
  - CPU (`apply_chain_with_mixes`): a slice starting at a snapshot whose strength slot is the next
    stop and has k = 0 (`zero_slice`) is not run when the dry frame is already at the output raster
    (strength stops are at 1×, so the wet frame could not have another size); the mix still runs on
    (dry, dry) — `dissolve_into` at k = 0 takes nothing from the wet side, so the bytes are the full
    path's. GPU (`chain_ops`): at a `Mix` with k = 0 for the one open snapshot whose size the slice
    did not change, the slice's ops are truncated; the snapshot and the mix stay, the alpha fold keeps
    what the slice declared. Not a per-chain strip: dropping the slots would let neighbouring geometry
    runs fuse across the removed stop and move bytes.
  - t945 (CPU, next to the owner's case in `morph_grid_cost.rs`, which has the helpers; not
    `morph_slice_render.rs`): at t = 0 (fade-in, strength 0) the frame equals the frame without the
    layer and the cost is 0 sub-samples, 1 snapshot, 1 mix; one frame in, the grid runs. RED without
    the skip: "(518400, 1, 1)". t945b (GPU, `gpu_render.rs`, on the adapter): the GPU frame equals
    the GPU frame without the layer, no sub-samples, CPU vs GPU within GEOM. RED without the
    truncation: "(147456, 1)". t946's range hash unchanged; t267/t267b/t268 and the kt-gpu suite
    (t181) green.
- [x] 2.2 Commit stage 2
  - Workspace green except t663 (the 60 s timeout under the full-run load, green in the kt-engine
    run just before).

## 3. One tile, copied

- [x] 3.1 The builder's `TileSpec` mark and the CPU tile path in `src/libs/kt-fx/src/geom.rs` (design D2). Write t943 (the sweep, including 3 taps per axis and the "step after the grid" refusals). Verify it is green. If any combination differs, unmark it, record the combination here, and re-run until green
  - `GeomProgram::tile: Option<TileSpec>` set by `grid_tile`: the grid is the LAST step of the series,
    k_out = 1, constant taps of 1, 2 or 4 per axis, more than 4 cells, no point-wise op on the output
    side. `exec_geom_cpu` computes the cell at the grid's origin with the same per-pixel code
    (`geom_pixel`) and copies it; pixels outside the grid take the full path. NO RING is needed: a
    pixel inside the grid has every sub-sample inside its own cell (x ∈ (ox−offx, ox−offx+1)).
    `sample_cost` = one cell + the field outside the grid (clipped to the frame: a grid larger than
    the frame, t299, underflowed the first version).
  - t943 (`grid_cost.rs`): 504 combinations (3 frame sizes incl. odd, 1× and 2× input, centred or
    not, 7 grids incl. one larger than the frame, alone / after a mirror / after a noise / inside
    a grid) are executed marked and unmarked — bytes equal in every one; refusals: exactly 3 taps
    (a 300×170 input into 320×180, asserted to BE 3 taps), a geometry step after the grid (with a
    point-wise op before it), 2×2, 1×1; a trailing noise is its own barrier pass. RED with an
    off-by-one in the copy: "the tile TileSpec { … cw: 26, ch: 6, cols: 12, rows: 30 } differs
    from the full pass". No combination had to be unmarked.
- [x] 3.2 The GPU tile pass and replicate pass in `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` + `src/libs/kt-gpu/src/engine.rs`, edited in the same commit as 3.1's `geom.rs`. Write t944 in `src/libs/kt-gpu/tests/parity.rs`. Verify t944 is green, and t296, t298, t331, t333, t181 and the parity suite are green
  - `GeomProgram::tile_program()` (kt-fx): the same prims with the grid's translation moved to the
    cell at its origin, output cw×ch — bit for bit the full program's pixels there, in f64 and f32
    (dyadic sub-sample offsets). kt-gpu: `FxOp::GeomTile { prog, tile }` = the fused shader into a
    pooled cw×ch target + the replicate pass `shaders/grid_tile.wgsl` (integer `textureLoad`,
    opaque black outside the grid — the Fold's `Mapped::Black`); `passes() = 2`,
    `slot_units() = GEOM_SLOT_UNITS + 1`, `PIPE_COUNT` 22. `geom_fused.wgsl` itself is unchanged.
  - t944 (`parity.rs`, on the adapter): the GPU tile equals the GPU FULL pass byte for byte on the
    same device — reached with no switch: two horizontal mirrors after the grid compose to an exact
    identity (the same prims) but unmark the program; CPU vs GPU within BILINEAR (t181's class for
    the grid); the op list is 2 passes vs 1. RED with the replicate shifted one texel: "12x30: the
    GPU tile differs from the GPU full pass". The kt-gpu suite (t181, t182, t183, t184, t296/t298
    geo cases, t309, t329) green with `KT_REQUIRE_GPU=1`.
- [x] 3.3 Verify the "after" halves of t942 and t946 are green, and the goldens (t26 group, t715, `GOLDEN_VIDEO`, t11, t67, t184) are green with no regeneration
  - t942's after half: 12×10 = 80×54×16 = 69 120, 12×30 = 80×18×16 = 23 040 sub-samples (was
    8 294 400 both); 1×1 and 2×2 unchanged. t946's range hash UNCHANGED (0xc73e46f0e915bf00) and the
    owner's-case suite runs in 0.43 s instead of 5 s. t946b (no longer ignored) is narrowed to what
    holds: at 12×30 and 12×10 a frame reads under 1/100 of the full pass, and no frame reads the full
    pass. The design's "peak ≤ 1/10" did not hold: when the automated grid passes through EXACTLY
    3 cells on an axis that axis has 3 taps, the tile is refused, and those transient frames keep the
    full pass (6 220 800 sub-samples at 5.5 s, 3×4 taps) — the range's new peak. t947 follows (peak
    below the full pass, equal on both backends). Goldens (t26 group, t715, GOLDEN_VIDEO, t11, t67,
    t184) green with no regeneration.
- [x] 3.4 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)
  - Workspace green except t476 (load flake, green alone); clippy without new warnings.

## 4. The panel

- [ ] 4.1 Performance panel: the "Compose" line (last frame and 1 s average in ms) next to the FX backend line, and the CPU line names the fallback reason. New `[perf]` keys go into all five catalogs, and `python3 scripts/subset-fonts.py` is run for the new glyphs. Write the panel half of t947 in `src/apps/kt-app/src/layout_tests.rs` (harness playback, no repaint after Stop). Verify t947 is green, t289/t100/t125/t128/t171 are green, and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH line
- [ ] 4.2 Live scenario `src/scripts/live/morphgrid.py`: the owner's case in a real window with the panel open, one `kt_live.step` screenshot per step of the PATH. Verify `./scripts/live-check.sh morphgrid` is green
- [ ] 4.3 Repeat 1.4 on a release build after stages 2–3, record the before/after table here, and send it to the owner with the short answer
- [ ] 4.4 Commit stage 4

## 5. Records and finish

- [ ] 5.1 Append the "why" to `openspec/specs/video-fx/design.md` (the tile rule, its exactness gate, why not a mip) and `openspec/specs/video-plan/design.md` (strength 0, FrameCost). Add the tile rule's pair note to `src/libs/kt-fx/CLAUDE.md`. Verify `openspec validate --specs` is green
- [ ] 5.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the live screenshots here
