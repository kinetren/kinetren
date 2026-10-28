# Tasks

Test numbers t942–t947 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done. Speed is judged only on a
release build.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's item 8 of ktprompts v26.10.18 is in `docs/user-feedback.md` (round of 2026-10-27), the short answer is in design.md, and the change's artifacts are written. Verify `openspec validate morpher-event-grid-cost --strict` is green

## 1. Measure first (no behaviour change)

- [ ] 1.1 `GeomProgram::sample_cost()` (pure) and `FrameCost` filled by the CPU slices (`video.rs`) and the GPU op list (`engine.rs`) (design D1). Write t942 with today's numbers as the "before" half (1×1, 2×2, 12×10, 12×30 at 960×540: 518 400 / 2 073 600 / 8 294 400 / 8 294 400). Its "after" half is RED. Verify the before half is green and record the RED line here
- [ ] 1.2 Write t946 in a new `src/libs/kt-engine/tests/morph_grid_cost.rs` (the owner's case built in code, design D4). Record the range hash of the CURRENT code into the test before any executor changes. Verify the rebuild and hash assertions are green and the per-frame cost bound is RED
- [ ] 1.3 `kt-cli probe --time`: the video section (frames, compose ms p50/p95/max, backend, passes and sub-samples per frame). Write the probe half of t947 in `src/apps/kt-cli/tests/cli.rs` (it checks fields and counts, not times). Verify it is green with `KT_VIDEO_GPU=0` and with the default
- [ ] 1.4 Investigation on a RELEASE build (`cargo build --release -p kt-cli`): run the t946 project through `kt-cli probe … --time`, saved by the test helper into the scratch dir. Run it once on the GPU and once with `KT_VIDEO_GPU=0`, for grids 1×1, 2×2, 12×10 and 12×30. Record compose ms, passes and sub-samples here, and state which of the three kinds of the short answer dominates on each backend. Send the owner Open Question 1 with these numbers
- [ ] 1.5 Commit stage 1

## 2. Strength 0 skips the layer

- [ ] 2.1 CPU slices and the GPU op list skip the slice when the slot's `k == 0` (design D3). Write t945 in `src/libs/kt-engine/tests/morph_slice_render.rs` on the CPU and the GPU. Verify t945 is green, t267/t267b/t268/t181 are green, and t946's hash is unchanged
- [ ] 2.2 Commit stage 2

## 3. One tile, copied

- [ ] 3.1 The builder's `TileSpec` mark and the CPU tile path in `src/libs/kt-fx/src/geom.rs` (design D2). Write t943 (the sweep, including 3 taps per axis and the "step after the grid" refusals). Verify it is green. If any combination differs, unmark it, record the combination here, and re-run until green
- [ ] 3.2 The GPU tile pass and replicate pass in `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` + `src/libs/kt-gpu/src/engine.rs`, edited in the same commit as 3.1's `geom.rs`. Write t944 in `src/libs/kt-gpu/tests/parity.rs`. Verify t944 is green, and t296, t298, t331, t333, t181 and the parity suite are green
- [ ] 3.3 Verify the "after" halves of t942 and t946 are green, and the goldens (t26 group, t715, `GOLDEN_VIDEO`, t11, t67, t184) are green with no regeneration
- [ ] 3.4 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)

## 4. The panel

- [ ] 4.1 Performance panel: the "Compose" line (last frame and 1 s average in ms) next to the FX backend line, and the CPU line names the fallback reason. New `[perf]` keys go into all five catalogs, and `python3 scripts/subset-fonts.py` is run for the new glyphs. Write the panel half of t947 in `src/apps/kt-app/src/layout_tests.rs` (harness playback, no repaint after Stop). Verify t947 is green, t289/t100/t125/t128/t171 are green, and `src/scripts/specs-user-path.py --check` (t799) accepts the PATH line
- [ ] 4.2 Live scenario `src/scripts/live/morphgrid.py`: the owner's case in a real window with the panel open, one `kt_live.step` screenshot per step of the PATH. Verify `./scripts/live-check.sh morphgrid` is green
- [ ] 4.3 Repeat 1.4 on a release build after stages 2–3, record the before/after table here, and send it to the owner with the short answer
- [ ] 4.4 Commit stage 4

## 5. Records and finish

- [ ] 5.1 Append the "why" to `openspec/specs/video-fx/design.md` (the tile rule, its exactness gate, why not a mip) and `openspec/specs/video-plan/design.md` (strength 0, FrameCost). Add the tile rule's pair note to `src/libs/kt-fx/CLAUDE.md`. Verify `openspec validate --specs` is green
- [ ] 5.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the live screenshots here
