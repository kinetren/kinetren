# Tasks

Test numbers t1520–t1549 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done, no `Co-Authored-By` or other agent lines) and
`scripts/target-prune.py`.

## 0. Measure first

- [x] 0.1 `kt-cli preview`: the real player, a wall-clock output device, the panel's counts, stages
- [x] 0.2 The stage record (`GpuStages`, `VideoCost.plan_ns`)

## 1. The decoder

- [x] 1.1 A span across decode sizes through `read_video_span_qualities`, chunks of whole moments,
  a budget at each key's size, unfit keys stay queued; t1520–t1522 red on the old worker (D1)
- [x] 1.2 The consequence: t1524 through the real player, red on the old worker
- [x] 1.3 Tests of kt-engine, kt-gpu, kt-cli, clippy, commit, target-prune

## 2. The picture on the device, and the cut

- [x] 2.1 `present`, the display pass, the targets; t1525–t1527, t1525 red without the premultiply
- [x] 2.2 `Player::present_preview`, `preview_display.rs`, `draw_preview`, the dump's
  `compose.on_display`, asserted by the live `gpufx`
- [x] 2.3 The first frame of a cut past the window (D3); t1528
- [x] 2.4 The real program measured under Xvfb, `submit` split out of `encode`
- [x] 2.5 Tests of kt-engine, kt-gpu, kt-app, kt-cli, clippy, commit, target-prune

## 3. The CPU path

- [x] 3.1 `compose_ahead.rs`: workers over a shared renderer, the stamp, the pool, `take` (D4)
- [x] 3.2 A miss holds, workers start past what cannot arrive, the window starts at their frontier
- [x] 3.3 The audio underruns of 26–57 s traced to Playback requests behind the window (D5); t1534
  red with the bug put back, after a first version that stayed green
- [x] 3.4 A burst between the chunks of a span; t1529 red without it
- [x] 3.5 t1530–t1533
- [x] 3.6 The real program: llvmpipe against CPU workers; the software adapter stays on the GPU path
- [x] 3.7 The hold is bounded: t947 found a frozen picture in reverse, at rate 0 and on a clock that
  stands; t1536 red on an endless hold
- [x] 3.8 An empty post-FX memo is no video miss; t1535
- [x] 3.9 The kt-app suite six times clean, the whole workspace (t815 alone red: its 2 s write limit), clippy, commit, target-prune
