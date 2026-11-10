# Tasks: stream-history-on-device

## 1. Arbiter and the GPU start record (392, 398)

- [x] 1.1 `Piece::cut` submits and admits the next piece by the next pass's prediction (D6); t2356, t2253, t2250–t2255
- [x] 1.2 Compute host calibration and `GpuStart` (93) record; `kt-cli trace gpu` start delay per class (D7); t2357, t2362
- [x] 1.3 Commit, target-prune

## 2. The batched walk on the device (391)

- [x] 2.1 Shaders: vector edit (table, loss) and residual drop; t2352
- [x] 2.2 `walk.rs`: the recorder, batches, readback rule by `shown_from`, continuation entries; per-position reference kept for tests; `StreamWalk` (94) record; t2350, t2351, t2363, t1239, t2270–t2278
- [ ] 2.3 kt-cli stream-cost prints positions/s, readbacks and jobs; measure before/after
- [ ] 2.4 Commit, target-prune

## 3. Memo covers the walk (394, 395 re-walk)

- [x] 3.1 `StreamMemo` pinned tier (rank thinning) and view; t2354
- [x] 3.2 Checkpoints in the walk; `needs`/`clean_at` accept recon-only entries; t2353
- [x] 3.3 `StreamKernel::walk_from`, sizing adds the checkpoint tier; player sets the view; prefetch advances a run as one walk; t2355, t2360, t2275, t2276
- [ ] 3.4 Commit, target-prune

## 4. V-fps and the marker (393, item 6)

- [ ] 4.1 `Composed.held`; app counts new pictures, held/s; `frame_show` held flag; t2358, t2361
- [ ] 4.2 Catching-up marker, catalogs (five languages); t2359, t100/t125
- [ ] 4.3 `kt-cli trace loop` computed vs distinct positions per lap; t2364
- [ ] 4.4 Commit, target-prune

## 5. Measure and close

- [ ] 5.1 Release, llvmpipe, his relinked copy: positions/s playing and paused, device share, computed vs distinct per lap, a jump's re-walk; `trace loop`/`gpu` before/after; what to expect on a card
- [ ] 5.2 Live `idle`, `heavy_loop`, `datamosh`; workspace tests of touched crates, clippy
- [ ] 5.3 found-defects rows removed/added (400+), whats-new line, crate CLAUDE.md notes
- [ ] 5.4 Archive (specs merged, design "why" into capability design.md); commit, target-prune
