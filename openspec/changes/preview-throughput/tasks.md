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
