# Tasks: gpu-leftovers

## 1. The compute host (511, 520)

- [ ] 1.1 `compute.rs`: the calibration and the warm-up dispatch admitted on the ring; t2510
- [ ] 1.2 `compute.rs`: one uniform arena a job, dynamic offsets, a bind group per (shader, binding size, buffers) within the job; t2511
- [ ] 1.3 Commit, target-prune

## 2. The readback memo (521, 523)

- [ ] 2.1 `pool.rs`: LRU by a generation counter; only complete composites kept; t2512, t2513
- [ ] 2.2 Commit, target-prune

## 3. The run form is named (510)

- [ ] 3.1 `compiled.rs`/`runs.rs`/`engine.rs`: the trial as a shadow pass; output form = the shape's state; the process-wide forms revision in the memo and present keys; t2514, t2515
- [ ] 3.2 Export composes runs settled (fused, compiled in the frame, no trial); t2516
- [ ] 3.3 Commit, target-prune

## 4. The ring's room by the display's cost (491)

- [ ] 4.1 `arbiter.rs`: the display's present cost remembered; a lower class's room is the period it leaves; t2517
- [ ] 4.2 Commit, target-prune

## 5. Export and disk tiers (522, 524)

- [ ] 5.1 `export.rs`/`export_pipeline.rs`: an incomplete frame's missing frames decoded alone and the frame composed again before it is written; t2518
- [ ] 5.2 `reel_preview.rs`: the strip key's version; t2519
- [ ] 5.3 Commit, target-prune

## 6. Close

- [ ] 6.1 found-defects rows removed, new rows 560+; whats-new; specs' tests of touched crates, clippy
- [ ] 6.2 Archive
