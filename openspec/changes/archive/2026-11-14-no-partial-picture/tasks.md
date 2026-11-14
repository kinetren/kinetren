# Tasks: no-partial-picture

## 1. The executor stops where it knows

- [x] 1.1 `kt-engine` `FramePlan::starved`; `PresentOptions::show_incomplete` removed; contract docs; t2531
- [x] 1.2 `kt-gpu` `present`/`execute`: a starved plan before the upload, an incomplete one after the resolve, no bytes from `execute_into`; t1526 rewritten, t1527, t2530
- [x] 1.3 Callers: preview bench, `kt-cli gpushare` (latency of drawn frames only), tests

## 2. The app draws only complete pictures

- [x] 2.1 `draw_preview`: no `shown_any`; the scrim stands in the frame's place with or without a picture; t2533
- [x] 2.2 The scrub meter: `sample_levels_at` → `Option`, the meter falls on an unknown level; t2532
- [x] 2.3 Live `open_heavy`: no picture under Loading on a fresh open, the open's work printed (t2534)

## 3. Close

- [x] 3.1 Class walk in design.md; found-defects 461 out, 570–573 in; whats-new; registry M1
- [x] 3.2 Tests of the touched crates, clippy, live `open_heavy` + `idle`; load before/after in design.md
- [ ] 3.3 Commit, target-prune, archive
