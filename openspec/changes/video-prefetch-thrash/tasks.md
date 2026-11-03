# Tasks: video-prefetch-thrash

Test numbers t1416–t1418 (with t1416b, t1417b, t1418b) are used by this change (t1419 is left to the other reservations).

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

- [x] 2.4 The preview's Playback request withdraws the frames it left (`compose_preview_measured`); the window starts at the frame on screen; t1418b
  - Done. Found by the first re-shoot (task 3.2): 59 no-read drops and "decoding cannot keep up"
    left on the fixed build. t1418b red with the withdrawal off (all eight frames between
    decoded), green after

- [x] 2.5 Thrash = dropped while still wanted: the pass number on the entry, `abandoned_evictions`; t1417b
  - Done. Found by the second re-shoot: 164 drops left. A temporary eviction log during the shot:
    0 still wanted, 52 abandoned (42 of the window the seek left). t1417b red under the old rule

## 3. Close

- [x] 3.1 Crate CLAUDE.md lines, What's new, `cargo test --workspace`, clippy
  - Done. kt-cache 11, kt-engine all green, clippy clean on the touched crates; the workspace runs
    in finish.sh
- [x] 3.2 The README diagnostics image re-taken on the release build, looked at
  - Done in five takes. The full `readme-shots.py` run failed twice at the hero step's first
    frame and once at its warm-up under load 40–111 from parallel sessions (the old binary
    behaves the same under load), so the image was taken by the pipeline's own `perf()` step
    alone (a scratch driver around `readme_shots.Shooter`, the crop of `publish`), gated on a
    load under 12–16. Take 1 (fix 2.1–2.3): no-read drops 59, V-fps 25.7 (before: 543, 13.9) →
    task 2.4. Take 2: 164 → the eviction log of 2.5. Take 4 (2.5): 0, with three 9–13 s UI
    stalls at load 15–20 that the next take at load 6.7 did not repeat. The committed take:
    "no-read drops total: 0", no thrash in the problem log; it keeps one callback underrun and
    "video hit 78 % — decoding cannot keep up" (4K at decode ×2 on llvmpipe), which are true
- [ ] 3.3 Archive (spec merged), `./scripts/finish.sh`, the version in the report
