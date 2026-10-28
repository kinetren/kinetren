# Tasks

Test numbers t912–t917 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done). t696, t696b and
`t238_warped_clip_is_never_sliced` are inverted by the owner's claim (design Open Question 4), not deleted.

## 0. Records before code

- [x] 0.1 The owner's claims (ktprompts v26.10.18 items 1–3) are recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this change's artifacts written. Verify `openspec validate slowed-reverb-is-an-event --strict` is green

## 1. Reproduce before fixing

- [ ] 1.1 Write t917 in `src/apps/kt-app/src/reach_tests.rs` with the spec's PATH (slowed 0.8× + speed curve, linked pair; Ctrl+C/V, S, Ctrl+X/V, a region copy). Run it on the current code and record here which gesture fails and how (design R3'); if all pass, record that and ask the owner Open Question 2 with the harness screenshots
- [ ] 1.2 Write t913 in `src/libs/kt-model/tests/warp.rs`. Verify it is RED on the current code (shapes refused) and record whether the formula-layer part shows the suspected raw-vs-effective profile mismatch (design R2)
- [ ] 1.3 Write t915 (`src/libs/kt-model/tests/morph_slice.rs`) and t916 (`src/libs/kt-engine/tests/morph_slice_render.rs`). Verify both are RED against the veto (whole-body patch)

## 2. Speed takes every shape (R2)

- [ ] 2.1 The sampling function and `from_curves_rev` (`src/libs/kt-model/src/warp.rs`), `resolve_one` skipping link-free `rate` curves (`links.rs`), `check_fx_auto` without the speed veto (`project.rs`), t696 inverted (design D2). Verify t913, t696, t707, t270–t270e, t247, t247b and the warp tests in `src/libs/kt-model/tests/warp.rs` are green
- [ ] 2.2 kt-app: the segment menu and "shape of all segments" offer every shape for speed (`automation.rs`, `widgets.rs`); `shape_rate_linear` removed from the five catalogs. Write t914 with its PATH and invert t696b. Verify t914, t696b, `i18n_tests.rs`, `font_tests.rs` are green
- [ ] 2.3 Verify `GOLDEN_AUDIO`/`GOLDEN_VIDEO` (t11, t67) and t695b do not move
- [ ] 2.4 Release build: `kt-cli probe <project> <from> <to> --time` on a project from our fixtures with a 10-minute event whose speed curve is a zigzag. Verify the audio render keeps up with real time with the same margin as a linear curve (±5 %), record both numbers here; if not, add the cumulative table + binary search of design Risks and re-measure
- [ ] 2.5 Commit stage 2 (`cargo test -p kt-model -p kt-engine -p kt-app`)

## 3. The morpher (R1, R3)

- [ ] 3.1 The picker kind filter with `Both` (`fx_registry.rs`, `widgets.rs`, `morpher_editor.rs`; design D1). Write t912 with its PATH. Verify t912, t48 (the registry filter tests in `fx_registry.rs`), t217 and t757 are green
- [ ] 3.2 Remove the warp veto from `MorpherEvent::windows_on` (`src/libs/kt-model/src/project.rs`; design D3); invert `t238_warped_clip_is_never_sliced`. Verify t915, t916, t238, t237, t233, t236, t288, t555, t556, t267, t267b, t268 are green
- [ ] 3.3 Run t917 again. Verify green; any defect it found in 1.1 is fixed here with its own regression line in the spec scenario
- [ ] 3.4 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)

## 4. Live and records

- [ ] 4.1 Live scenario: extend `src/scripts/live/slowed.py` with the copy/paste/split gestures of t917 and `src/scripts/live/morph.py` with "+ effect…" → Pitch shift (`kt_live.step` screenshot per step). Verify `./scripts/live-check.sh slowed morph` is green
- [ ] 4.2 Update `src/libs/kt-model/CLAUDE.md` and `src/apps/kt-app/CLAUDE.md` where they repeat "speed is linear only" or "a warped clip is not cut"; run `src/scripts/specs-user-path.py --check` and `python3 src/scripts/features-index.py --check`. Verify both green; commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green; record the version `vYY.MM.DD-hash` and the live screenshots of 4.1 here; ask the owner to repeat items 1–3 on that build and record his answer in `docs/user-feedback.md`
- [ ] 5.2 After archiving: append the "why" (R1 oversight of R.35 S2; R2 the profile vs. the curve, D2 sampling; R3 the veto outlived R.39) and the revisions to `openspec/specs/automation/design.md` and `openspec/specs/morpher/design.md`
