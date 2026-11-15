# Tasks: loop-keeps-its-lap

## 1. The kept mix

- [x] 1.1 `Tenant::Mix` grows, first in the order of value; `grants` over N kinds (t2600)
- [x] 1.2 `MixBudget`: account + own want, `mix_want` by the loop; the store reads its bound at use, `fit` (t2602)
- [x] 1.3 `MixStore` keeps a prefix of the kept loop's laps; `write` says what it did not admit; trace aux `passed` (t2601, t2603)
- [x] 1.4 The player: the prefetch thread asks by the region, the render thread fits after a shrink and is told the loop at start (t2607)

## 2. The decoded-frame cache and the front memo

- [x] 2.1 `BurstCache::keep_loop` / `lap`: lap stamps, the window, the victim order (t2604, t2605)
- [x] 2.2 The player tells the cache the loop and the laps; the prefetch-driven loop past the bound (t2606)
- [x] 2.3 The front memo as a `StreamMemo`, told with the stream memo (t2608, row 542)

## 3. Records

- [x] 3.1 Class walk in `design.md`; found-defects rows 542, 550, 551 out; What's new; crate CLAUDE.md
- [x] 3.2 nextest of kt-time, kt-cache, kt-engine, kt-cli; clippy; load measured on a release build
- [x] 3.3 Commit, target-prune; archive
