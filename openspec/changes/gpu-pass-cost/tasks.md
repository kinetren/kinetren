# Tasks: gpu-pass-cost

## 1. The instrument (committed alone)

- [ ] 1.1 `kt_gpu::pass_time`: the timer per engine (query set, resolve, readback pool, no wait), the process-wide aggregate, `take()`/`Second::line()`
- [ ] 1.2 Every pass of `encode_jobs`/`encode_chain`/display/clear timed with its label; labels for fixed pipes, geometry, plugin taps and barrier passes (`BarrierKernel::pass_names`: VHS Tape, Camera ISP)
- [ ] 1.3 The compute host: a timestamp pair per dispatch, `Step::name`; Datamosh and the compute generators name theirs
- [ ] 1.4 Devices ask for `TIMESTAMP_QUERY` where the adapter has it: headless, export, compute, the eframe device; without it the submit-to-done time
- [ ] 1.5 Load watch: one `GPU` record per busy second; Diagnostics shows it; catalogs
- [ ] 1.6 `kt-cli preview` prints the summary
- [ ] 1.7 Tests t2020 (the lines sum to the frame's device time), t2021 (the journal holds the GPU record of a busy second), t2022 (nothing queried or recorded at rest)

## 2. The cost found and fixed

- [ ] 2.1 Rank every effect's passes and every shared pass on this machine with the instrument (heavy showcase, 1080p) and by reading the shaders
- [ ] 2.2 Fix the costly mechanisms within `FrameTol`
- [ ] 2.3 Guards t2023+ (no resource created per frame after warm-up, every kind; pass counts pinned)
- [ ] 2.4 The class walked and written down (design.md), the table before/after, what his card must confirm

## 3. Close

- [ ] 3.1 kt-gpu CLAUDE.md, specs, What's new, registry row H9, found-defects 108
- [ ] 3.2 Tests of the specs, clippy, build kt-app/kt-cli, live scenario, target-prune
