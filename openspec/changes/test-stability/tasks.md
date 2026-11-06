# Tasks: test-stability

## 1. Tests: a wait is bounded by a hang, not by the clock

- [x] 1.1 Baseline under load, before any edit: the reds and what they read
- [x] 1.2 `kt_time::test_deadline()`, `TEST_WAIT`; every wait that is not the check uses them
- [x] 1.3 `fixtures/gen.sh`: a lock and a run's own temp names; `harness_still_of` asks `fixture()`; t1670 red on the old script
- [x] 1.4 t815 on the thread's processor clock
- [x] 1.5 t1524 against a control, t1538 on a source that never answers
- [x] 1.6 Scene races: t362, t599, t836, t20, t1282, t874, t841
- [x] 1.7 Sleep-then-assert: t398b, the perf panel's t947, the rebuild check of t20
- [x] 1.8 t1671: the guard of clock limits

## 2. Live scenarios

- [ ] 2.1 `poll_step` and `wait_playhead` follow the frame time; `wait_gone` for what closes
- [ ] 2.2 `saved_project` is `save_now`, and the save waits on a fresh dump
- [ ] 2.3 `winshell` counts frames; the read of the notices window waits for a fresh dump
- [ ] 2.4 ui-probe's walk survives a window that closes
- [ ] 2.5 `live-check.sh` finds a free display and holds it, the workers' output streams
- [ ] 2.6 Every scenario red under load: cause, then fix

## 3. Cost of the live run

- [ ] 3.1 `still` and `screen_diff` compare from X
- [ ] 3.2 Measure `readme_shots` and the whole run

## 4. Proof and hand-over

- [ ] 4.1 Full run green twice in a row under a parallel build (nextest, as `finish.sh`)
- [ ] 4.2 `cargo test --workspace` green twice in a row under the same load
- [ ] 4.3 `live-check.sh` under load, every scenario green with no rerun
- [ ] 4.4 Rows of `docs/found-defects.md`, the measurements in design.md, CLAUDE.md notes
- [ ] 4.5 Archive the change into the specs
