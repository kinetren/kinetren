# Tasks

- [x] 1.1 Re-measure row 11 from the first moment, before and after `preview-throughput`
- [x] 1.2 The start's wait, its limit, every kind of start
- [x] 1.3 The callback's empty pass before the first frame; `OutputEdge::started`
- [x] 1.4 `kt-cli preview --cold`; t1537 red without the wait, t1538
- [x] 1.5 18 cold starts of the real program
- [x] 1.6 The whole workspace, clippy, live scenarios, commit, target-prune

Workspace: 209 targets green, t815 red (its 2 s write limit, known). Live: gpufx, seekjump, loop, rate,
slowedcut, pitchtail, render green; `loop` once on the second try, from two reads of one 10 Hz
geometry dump, then four clean runs. What's new extended.
