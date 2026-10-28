# Tasks

Test numbers t912–t917 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done). t696, t696b and
`t238_warped_clip_is_never_sliced` are inverted by the owner's claim (design Open Question 4), not deleted.

## 0. Records before code

- [x] 0.1 The owner's claims (ktprompts v26.10.18 items 1–3) are recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this change's artifacts written. Verify `openspec validate slowed-reverb-is-an-event --strict` is green

## 1. Reproduce before fixing

- [x] 1.1 Write t917 in `src/apps/kt-app/src/reach_tests.rs` with the spec's PATH (slowed 0.8× + speed curve, linked pair; Ctrl+C/V, S, Ctrl+X/V, a region copy). Run it on the current code and record here which gesture fails and how (design R3'); if all pass, record that and ask the owner Open Question 2 with the harness screenshots
  - RED on the old code, and NOT where the design expected: Ctrl+C / Ctrl+V and the region copy
    worked; **S on a slowed event with a speed curve was refused** — `Split` gave
    `InvalidLayout(BadFxAuto)`: `clip_slice` moves a warped piece's keys by the anchor's delta,
    with a speed below 1 at the start the delta is negative, the first key landed at −45 000 ticks,
    and `check_fx_auto` rejected any key before the material start. The status line said it, no
    note. This is the owner's "does not let me cut it". The program has no Ctrl+X: the PATH and
    the test use Ctrl+C → Delete → Ctrl+V (the spec is corrected). Open Question 2 is answered by
    the test itself.
- [x] 1.2 Write t913 in `src/libs/kt-model/tests/warp.rs`. Verify it is RED on the current code (shapes refused) and record whether the formula-layer part shows the suspected raw-vs-effective profile mismatch (design R2)
  - RED: every shape refused (`RateShapeMustBeLinear`), and the suspected neighbour CONFIRMED —
    with a formula layer `1 + 0.5·sin(6t)` the raw event consumed 720 000 ticks (base 0.8, the
    layer ignored) while the formula integrates to ~914 600: the interface's map did not see what
    the render resolved.
- [x] 1.3 Write t915 (`src/libs/kt-model/tests/morph_slice.rs`) and t916 (`src/libs/kt-engine/tests/morph_slice_render.rs`). Verify both are RED against the veto (whole-body patch)
  - Both RED against the veto: one piece instead of three in every case of t915; t916 — the sound
    and the frames outside the window differ (the whole body was patched).

## 2. Speed takes every shape (R2)

- [x] 2.1 The sampling function and `from_curves_rev` (`src/libs/kt-model/src/warp.rs`), `resolve_one` skipping link-free `rate` curves (`links.rs`), `check_fx_auto` without the speed veto (`project.rs`), t696 inverted (design D2). Verify t913, t696, t707, t270–t270e, t247, t247b and the warp tests in `src/libs/kt-model/tests/warp.rs` are green
  - `rate_profile_keys` (warp.rs) + `RATE_GRID_TICKS` = 450 (5 ms, not the proposed 10 ms: on
    `1 + 0.5·sin(6t)` the 10 ms trapezoid left 4.4 ticks of error over 10 s, over the one-audio-frame
    bound; 5 ms leaves ~1.1); `from_curves_rev` samples `rate` and keeps a key-less formula layer;
    `resolve_one` skips link-free `rate`; the `RateShapeMustBeLinear` branch and the variant are
    removed; t696 inverted. Plus the split fix of 1.1: `keys_may_precede_material` — a negative
    key is legal on a WARPED event only, so t190 ("before the start of the material — refused")
    stays as it is for an event without a warp.
- [x] 2.2 kt-app: the segment menu and "shape of all segments" offer every shape for speed (`automation.rs`, `widgets.rs`); `shape_rate_linear` removed from the five catalogs. Write t914 with its PATH and invert t696b. Verify t914, t696b, `i18n_tests.rs`, `font_tests.rs` are green
  - The refusal branch of the segment menu and of `shape_all_control` is gone, `shape_rate_linear`
    removed from the five catalogs. t696b is the speed part of t698 (`layout_tests.rs`), inverted
    there. t914: the menu of the speed curve lists every shape and Formula; Smooth and Formula
    are one undo step each, and the material at a quarter of the event is equal for the raw event
    (timeline) and the effective one (render) — a symmetric shape eats the same over its whole
    segment, so the quarter, not the end, shows the shape. "Shape of all segments" is driven on
    the widget itself: egui's submenu closes on the harness click that would open it.
- [x] 2.3 Verify `GOLDEN_AUDIO`/`GOLDEN_VIDEO` (t11, t67) and t695b do not move
  - Green in every workspace run of this change (t11, t67 in `golden_export.rs`, t695b).
- [x] 2.4 Release build: `kt-cli probe <project> <from> <to> --time` on a project from our fixtures with a 10-minute event whose speed curve is a zigzag. Verify the audio render keeps up with real time with the same margin as a linear curve (±5 %), record both numbers here; if not, add the cumulative table + binary search of design Risks and re-measure
  - Projects: one 600 s audio event (synth sine) with slowed and a speed curve 0.6→1.4, straight
    vs a 60-period triangle zigzag; `kt-cli probe … 290 320 --time`, release. FIRST: 78 ms vs
    **1558 ms** per 30 s (20×) — not within ±5 %. Cause (no `perf` rights here, found by reading):
    per burst kt-engine recomputed the source-window end and the mirror axis, each walking the
    whole sampled profile, and `param_at` walked it per chunk. Fixed: `param_at` by binary search
    (same segment, same bits) and `AudioClip::src_ends` computed on the first render. AFTER, on
    the 5 ms grid: straight 79 ms, zigzag 83 ms (+5 %); a prefetch pass 0.8 ms; looped 316 vs
    325 ms per 120 s.
- [x] 2.5 Commit stage 2 (`cargo test -p kt-model -p kt-engine -p kt-app`)
  - Committed together with stage 3 (one commit "stages 1–3"): the split fix of 1.1 and the
    morpher changes touch the same tests, and `cargo test` must be green on every commit.

## 3. The morpher (R1, R3)

- [x] 3.1 The picker kind filter with `Both` (`fx_registry.rs`, `widgets.rs`, `morpher_editor.rs`; design D1). Write t912 with its PATH. Verify t912, t48 (the registry filter tests in `fx_registry.rs`), t217 and t757 are green
  - `PickKinds { Video, Audio, Both }` with `From<bool>` (the event and track windows unchanged);
    an audio row in the gallery has no thumbnail strip. t912 green, and RED with the morpher
    passing `false` again ("the morpher gallery does not show the audio kind “compressor”" —
    reverb was found even before, slowed reverb is also a video kind).
- [x] 3.2 Remove the warp veto from `MorpherEvent::windows_on` (`src/libs/kt-model/src/project.rs`; design D3); invert `t238_warped_clip_is_never_sliced`. Verify t915, t916, t238, t237, t233, t236, t288, t555, t556, t267, t267b, t268 are green
  - The veto is removed; `t238_warped_clip_is_never_sliced` inverted. **t288 part 3 inverted too**:
    it restated the same veto ("a covered stretched clip is patched whole and is NOT cut (R.37)") —
    the owner's decision on t238 is the decision on this rule; parts 1–2 (REACH) unchanged; the
    morpher delta gains the MODIFIED requirement holding that scenario. t916 measured: the frames
    outside the window are byte-for-byte; the sound before the window too (up to the chunk holding
    the seam); after the window a sinc restart — 67 frames at a constant speed, then equal bytes;
    on a speed curve the chunk grid of the new piece starts at its own edge and the material lags
    ~10 audio frames (0.2 ms). A plain split S of such an event has the same lag (an engine
    property of per-chunk speed, not of the layer). The spec states this measure instead of
    "byte-for-byte".
- [x] 3.3 Run t917 again. Verify green; any defect it found in 1.1 is fixed here with its own regression line in the spec scenario
  - Green (the split fix is in 2.1); the spec scenario names the defect: "the split was refused
    before this change … `BadFxAuto`".
- [x] 3.4 Commit stage 3 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)
  - `cargo test --workspace --no-fail-fast`: 1160 passed; red only `t640` (`sidecar open failed:
    sqlite: database is locked` under load) and `t815` ("writing the restore record took 3.7 s") —
    both load-timed, green alone, on paths this change does not touch (t640 was red under load in
    the previous round too). Clippy: one new error (an approximation of TAU in t913's oracle) fixed;
    the rest is older.

## 4. Live and records

- [x] 4.1 Live scenario: extend `src/scripts/live/slowed.py` with the copy/paste/split gestures of t917 and `src/scripts/live/morph.py` with "+ effect…" → Pitch shift (`kt_live.step` screenshot per step). Verify `./scripts/live-check.sh slowed morph` is green
  - `slowed.py` already cut a CONSTANT 0.5× event (that always worked), so the owner's case went
    into a new scenario `src/scripts/live/slowedcut.py`: an event with a speed curve 0.5→1.5,
    Edit → Copy / Paste (the virtual screen has no system clipboard — the menu calls the same body,
    as `nestshare` does), then S at 3 s: green, the frame at 4.5 s unchanged by the cut. `morph.py`
    + "+ effect…" → "pitch" → Pitch shift → the patch's audio chain holds it. `./scripts/live-check.sh
    slowedcut morph slowed`: 3 of 3 green. Screenshots: `target/live-out/slowedcut/01-step-open,
    02-step-pasted, 03-step-cut`; `target/live-out/morph/06-step-morph-window,
    07-step-morph-picker-pitch, 08-step-morph-pitch-picked`.
- [x] 4.2 Update `src/libs/kt-model/CLAUDE.md` and `src/apps/kt-app/CLAUDE.md` where they repeat "speed is linear only" or "a warped clip is not cut"; run `src/scripts/specs-user-path.py --check` and `python3 src/scripts/features-index.py --check`. Verify both green; commit stage 4
  - kt-model CLAUDE.md: the veto notes rewritten, a new section "Speed takes every shape";
    kt-fx CLAUDE.md: the `ease on rate` line. kt-app CLAUDE.md had no such rule. `specs-user-path.py
    --check` and `features-index.py --check` green.

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green; record the version `vYY.MM.DD-hash` and the live screenshots of 4.1 here; ask the owner to repeat items 1–3 on that build and record his answer in `docs/user-feedback.md`
- [ ] 5.2 After archiving: append the "why" (R1 oversight of R.35 S2; R2 the profile vs. the curve, D2 sampling; R3 the veto outlived R.39) and the revisions to `openspec/specs/automation/design.md` and `openspec/specs/morpher/design.md`
