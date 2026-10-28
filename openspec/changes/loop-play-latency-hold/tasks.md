# Tasks

Test numbers t900–t905 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done).

## 0. Records before code

- [x] 0.1 The owner's complaint (kkt 2026-10-27 item 1) is recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this change's artifacts written. Verify `openspec validate loop-play-latency-hold --strict` is green

## 1. Prove the cause before the fix

- [ ] 1.1 Write t901 and t903 in `src/libs/kt-engine/tests/frame_order.rs` against the CURRENT release rule (`(generation, wraps)`), with the deterministic callback/UI schedule of design D4. Verify both are RED for every latency > 0 with Play from outside, and record here the longest hold per (quantum, latency) — it must match the table in design.md; if it does not, stop and revise the design
- [ ] 1.2 Add the output latency (ms, from `latency_smooth`) and the last callback size (frames) to the perf panel's audio rows in `src/apps/kt-app/src/app.rs` and the five catalogs (a permanent diagnostic; `i18n_tests.rs`/`font_tests.rs` stay green). Verify `cargo test -p kt-app i18n font` is green
- [ ] 1.3 Probe on wine: build the Windows zip (`./scripts/release-build.sh --target windows`), and on a wine machine WITH an audio device (the owner's; the build container has none) open a copy of `src/fixtures/demo.ktproj` (our fixture, not the owner's project), draw a region, place the cursor before it, press Play; record the perf panel's latency, callback size and the "V skipped" rollback count during the freeze, plus the black box (`Ev::Wrap`/`Ev::Stall`). Verify the rollback count grows during the freeze (the hypothesis) and write the numbers here; if it does not grow, the cause is elsewhere — stop and revise

## 2. The display lap

- [ ] 2.1 Write t900 and t902 in `src/libs/kt-engine/src/player.rs` (pure functions). Verify they are RED
- [ ] 2.2 Implement the display lap and the jump clamp in `src/libs/kt-engine/src/player.rs` (design D1–D3: the lap reported with the frame, forward-only within a generation, the target clamp after a jump kept in the `video_clock` state). Verify t900, t902, t22, t44, t56, t114, t134 are green
- [ ] 2.3 Switch `draw_preview` (`src/apps/kt-app/src/app.rs`) to release by `(generation, display lap)`. Verify t901, t903 (now green), t117 and t114 are green
- [ ] 2.4 Write t904 (live player, latency 9 600 frames injected before Play, three laps from a cursor before the region). Verify green on a debug build three runs in a row (it is timing-based like t117)
- [ ] 2.5 Commit stages 1–2 (`cargo test -p kt-engine`, `cargo clippy -p kt-engine --all-targets` with no new warnings)

## 3. The program

- [ ] 3.1 Write t905 in `src/apps/kt-app/src/layout_tests.rs` with the spec's PATH (harness: inject the latency, region by a ruler drag, cursor before it, Space, three laps). Verify green, and RED with stage 2.3 reverted
- [ ] 3.2 Extend the live scenario `src/scripts/live/loop.py` with "Play from outside the region": cursor before the region, Space, `kt_live.step` screenshots at the start, mid-lap and after two folds; the preview colour must change with the region's clips. Verify `./scripts/live-check.sh loop` is green (on a machine with an audio device; the scenario reports "no playback" otherwise)
- [ ] 3.3 Correct `src/apps/kt-app/CLAUDE.md` line 23 ("a rollback ≤2 cells is ignored" — the ratchet has had no size tolerance since R.14 item 14) and name the display lap there. Verify `features-index.py --check` and `specs-user-path.py --check` are green
- [ ] 3.4 Run the UI guards of the ui-reach and ui-timeline specs (`reach_tests.rs`, `layout_tests.rs`) and `cargo test --workspace --no-fail-fast`. Verify green; commit stage 3
- [ ] 3.5 Ask the owner to repeat item 1 on wine with the new build (Play from outside, a second region); record his answer in `docs/user-feedback.md`

## 4. Finish

- [ ] 4.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots of 3.2 here
- [ ] 4.2 After archiving: append the "why" (release by the display lap, not the head's wrap; the jump clamp; the wine latency numbers of 1.3) to `openspec/specs/transport/design.md`
