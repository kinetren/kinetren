# Tasks: video-prefetch-thrash

Test numbers t1416–t1418 are used by this change (t1419 is left to the other reservations).

## 1. Measure

- [x] 1.1 `kt-cli probe`: the video prefetch window against the cache at three preview sizes and
  a playback of the range with an infinitely fast decoder (`simulate_video_playback`)
  - Done. Showcase, 640×360: one pass 140 frames = 604 MB of 512 MB, 22 025 entries dropped
    before the first read; the old window counted one frame per tick (design, Measurements)

## 2. Fix

- [x] 2.1 The window in bytes of every layer, at whole frames, nearest first (`video_prefetch_wants`); t1416, t1416b
- [x] 2.2 `BurstCache::refresh`, the pass refreshes the window far → near (`video_prefetch_pass`); t1417
- [x] 2.3 Passed look-ahead withdrawn from the queue (`MediaWorker::withdraw_video`); t1418
  - Done. t1416/t1416b/t1418 red on the old window and a no-op withdraw, green after; t1417
    holds plain LRU's loss inside the test

## 3. Close

- [ ] 3.1 Crate CLAUDE.md lines, What's new, `cargo test --workspace`, clippy
- [ ] 3.2 The README diagnostics image re-taken on the release build, looked at
- [ ] 3.3 Archive (spec merged), `./scripts/finish.sh`, the version in the report
