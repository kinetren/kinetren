# Tasks: gpu-throughput

## 0. Baseline

- [ ] 0.1 Release kt-cli at the base; his relinked copy `problem_region_26_11_09` and `showcase --mosh`: positions/s playing/paused, admission share, cores used, laps until all-kept, display latency (`gpu-share`); `trace gpu`/`trace loop`

## 1. The ring admits by its predicted drain (480)

- [ ] 1.1 `arbiter.rs`: flights with submit instants, the FIFO drain, display next/end, lower-class rule, `AHEAD_PIECES`, waits with a deadline; cut at half the bound for a lower class; t2430, t2431, t2251, t2356, t2250, t2252
- [ ] 1.2 `Ring::note_display` from `present`/`present_kept`; t2433, t2253, t2254
- [ ] 1.3 Commit, target-prune

## 2. Cold passes are predicted (481)

- [ ] 2.1 `seeds.rs`: the disk table (load, note, save throttled, scale by work); engine Timer and compute host fall back to it; t2434
- [ ] 2.2 Compute job learns from its own pieces' timestamps; t2435
- [ ] 2.3 Commit, target-prune

## 3. Records (486)

- [ ] 3.1 Query-set pool per job; `gpu_start` per piece; `GpuExec` (98); t2437, t2357
- [ ] 3.2 `Edit.a` = model revision; t2439
- [ ] 3.3 `trace gpu` admission share and idle gaps; t2438, t2362
- [ ] 3.4 Commit, target-prune

## 4. Compute ahead and the processor (482, L4, 485)

- [ ] 4.1 Prefetch far part at class background, loop end, pause inside a loop; t2432, t2360, t2355
- [ ] 4.2 Preview sources convert on the render's threads; t2440, t1765
- [ ] 4.3 The rule's scenario: t2436
- [ ] 4.4 Datamosh convert on CPU: measure its share, decide, record in design D4
- [ ] 4.5 Commit, target-prune

## 5. Measure and close

- [ ] 5.1 Release, llvmpipe: the baseline's numbers after; what a real iGPU should show
- [ ] 5.2 Same bytes: t1239, t2270–t2278, t2350–t2364; workspace tests of touched crates; clippy
- [ ] 5.3 Live `idle`, `heavy_loop`, `datamosh`
- [ ] 5.4 found-defects rows 480–482, 485, 486 closed; new rows (490+); What's new line; crate CLAUDE.md notes
- [ ] 5.5 Archive (specs merged, design into capability design.md); commit, target-prune
