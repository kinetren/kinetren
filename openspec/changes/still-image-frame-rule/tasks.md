# Tasks

Test numbers t936–t941 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's item 9 of ktprompts v26.10.18 is in `docs/user-feedback.md` (round of 2026-10-27), and this change's proposal, delta, design and tasks are written. Verify `openspec validate still-image-frame-rule --strict` is green

## 1. Prove the cause, tests first

- [ ] 1.1 Confirm design D1's claim that nothing in the engine reads `MediaInfo::still`: `grep -rn "\.still\b" src/libs src/apps` lists only `kt-media/src/ffmpeg.rs` and `kt-cli/src/showcase.rs`. Then write t936 in `src/libs/kt-engine/tests/conform_rule.rs` against the REAL `SourceFacts::observe` (no hand-built facts), covering a square still, a SAR 2:1 still, a video, a generator, a title and the placeholder. Verify it is RED for the still (no view), record the failure line here, and verify t718 stays green
- [ ] 1.2 `src/fixtures/gen.sh`: `still_square.png` and `square.mp4` (design D3). Then write t937 and t941 in `src/libs/kt-engine/tests/stills.rs` (four rules plus the "project" follow; placeholder in 4:3; generator identity). Verify t937 is RED (every rule gives the canvas) and t941 is green before the fix
- [ ] 1.3 Write t940 in `src/apps/kt-cli/tests/cli.rs` (probe's conform line for the still and for the placeholder). Verify it is RED: the probe prints `view=180x180` today, but the renderer uses none. The test asserts that the probe line equals what the renderer reports, so the mismatch shows

## 2. One fact, one accessor

- [ ] 2.1 `SourceFacts::observe`: store the view only for a source with proportions (`fps.is_none() && !still` is shapeless). Placeholder `still: false` (D1). Add `SourceFacts::view_of` and the `lead_view` helper (D2), and delete the renderer's `else if procedural { None }` branch. Verify t936, t937 and t941 are green, and t796, t796b, t796d, t471, t712, t718 and t722 are too
- [ ] 2.2 `Player::source_view` and the `kt-cli probe` conform section call `view_of`/`lead_view`. Verify t940 is green and t290 (the probe test in `cli.rs`) is still green
- [ ] 2.3 Write t938 in `src/libs/kt-gpu/tests/gpu_render.rs` (still under fit on CPU and GPU over a coloured lower layer; stretch equals the earlier frame byte-for-byte on the CPU). Verify it is green with the tolerance class of t713, and t713/t714b/t715 and `GOLDEN_VIDEO` are green without regeneration
- [ ] 2.4 Commit stages 1–2 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)

## 3. What the person sees

- [ ] 3.1 Write t939 in `src/apps/kt-app/src/layout_tests.rs`: harness, square still on a 320×180 canvas, switch the rule through the FX window's row, and compare the on-screen frame rectangle with the preview pixels after each switch. Verify it is green and `src/scripts/specs-user-path.py --check` (t799) accepts its PATH line
- [ ] 3.2 Extend the live scenario `src/scripts/live/stills.py` with the square still in a 16:9 project. Take one `kt_live.step` screenshot per rule (fit, fill, stretch, as is), and measure the picture's width on screen as `conform.py` does. Verify `./scripts/live-check.sh stills` is green
- [ ] 3.3 Run the kt-app UI guards (`reach_tests.rs`, `layout_tests.rs`). Verify they are green
- [ ] 3.4 Commit stage 3

## 4. Records and finish

- [ ] 4.1 Append to `openspec/specs/video-plan/design.md` the "why": "procedural" (no grid) and "shapeless" (no proportions) are two facts, and R.67 had silently merged them. Name the three readers that had their own copy. Update the comment at `video.rs` ~932 that sends a still to "no proportions". Verify `openspec validate --specs` is green
- [ ] 4.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the screenshots of 3.2 here
