# Tasks

Test numbers t918–t923 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files: the checkout
may hold another session's uncommitted edits.

## 0. Records before code

- [x] 0.1 The owner's complaints (kkt item 2, ktprompts 26.10.18 item 6) are recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this proposal names `shared-window-shell`. Verify `openspec validate shared-window-shell --strict` is green

## 1. Find the growth first (investigation, no fix)

- [ ] 1.1 Write t923 in `src/apps/kt-app/src/layout_tests.rs` with the owner's morpher scene (batch layer, 12-slot strip, two area references, an open curve graph, a source field) at `with_pixels_per_point(1.25)` and `1.5`, 600 frames, the pointer moving over the main window and over the morpher window, logging the width each 50 frames. Verify: record here whether it is RED on today's code and the per-50-frame widths
- [ ] 1.2 Run the real program under Xvfb at the program's own zoom 1.1 and 1.2 (Ctrl+= pressed once and twice — the owner's case, 2026-10-28: Ubuntu, 1920×1080, native scale 1.0), and with `WINIT_X11_SCALE_FACTOR=1.25` as a second case (new live scenario `winshell` in `src/scripts/live/`, which the specs name once it exists; `kt_live.step` screenshots at 0 s, 10 s and 30 s of pointer motion), measuring the morpher window's width through `scripts/ui-probe.py`. Verify: record the widths here; if neither 1.1 nor 1.2 grows, stop and ask the owner design.md Open Question 1 before stage 4
- [ ] 1.3 Bisect in t923 (comment the parts out in a scratch copy, not in the commit): the window `vscroll`, then each group of the body (targets, source, chain strip, refs, curve block, thumbnail picker). Verify: the widget or option that feeds the width back is named here with its `file:line`, and design.md D3 gets the chosen variant (a/b/c) as a dated line

## 2. Tests first

- [ ] 2.1 Write t918 in `src/apps/kt-app/src/layout_tests.rs`: every openable registry window at scales 1.0/1.25/1.5, 300 frames with pointer motion, outer rect within one physical pixel; reuse the openers of t779. Verify it compiles and record which windows are RED today
- [ ] 2.2 Give t779 real openers for `morph`, `transition`, `camera` (scenes of t242, t227, t231b) in `src/apps/kt-app/src/reach_tests.rs`, removing their `OpenWin::Cant`. Verify t779 runs them and record whether the form contract holds for them today
- [ ] 2.3 Write t919, t920, t922 in `reach_tests.rs` with the `- **PATH**` of the spec. Verify: t919 is RED on `camera`, `transition`, `project_settings`, `env_fit`, `pub_unpublish` and the two modals; t920 is RED (fixed order closes FX before camera); t799 accepts the PATH lines
- [ ] 2.4 Write t921 in `reach_tests.rs` (scanner outside the shell module). Verify it is RED and lists the 23 sites and the four `vscroll(true)` calls of the inventory

## 3. The shell

- [ ] 3.1 Create the shell module (design D1): `Spec`, the closed `Kind` enum, `show` that sets id, cross, collapse, resize, the body scroll (`window_body` moved in) and the actions row, and returns open/closed-by/`LayerId`; the per-frame list of shown windows on `KtApp` (D2). Verify `cargo build -p kt-app` and `cargo clippy -p kt-app --all-targets` with no new warnings
- [ ] 3.2 Move the tool windows (`fx`, `morph`, `transition`, `camera`, `track_window` — dropping its own scroll copy, `published_win`, `kt_notices`, `kt_diag`, `kt_search`, `kt_ffmpeg`) onto the shell; the four `vscroll(true)` calls disappear. Verify t918 and t923 are green, t779 and t780 still green
- [ ] 3.3 Move the dialogs and progress windows (`env_fit`, `project_settings`, `kt_render_settings`, `kt_render_progress`, `pub_name`, `pub_unpublish`, `seq_name`, `paste_names`, `preset_save`, `kt_analysis`) and the two modals onto the shell, with their declared Esc action; delete the four per-dialog Esc handlers. Verify t250, t343, t345 and the publishing tests of `compositions` still green
- [ ] 3.4 Replace the window part of `escape_action` by "close the top shell window" (D2); keep step 0 and the non-window steps. Verify t919, t920, t922 and the Esc cascade tests (t636 and the `layout_tests.rs` cases calling `escape_action`) are green
- [ ] 3.5 Apply the width rule of D3 chosen in 1.3 inside the shell. Verify t918 and t923 green at all three scales
- [ ] 3.6 Update t757's scanner to the shell call; verify t757 and t921 are green and the `WINDOWS` registry is unchanged byte for byte
- [ ] 3.7 Run the UI guards of `ui-reach` and `ui-timeline` (`reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`, `i18n_tests.rs`), then commit stages 1–3 (`cargo test --workspace --no-fail-fast` green)

## 4. Live and records

- [ ] 4.1 Finish the `winshell` live scenario: at zoom 1.2 open the morpher window and move the pointer for 30 s (width unchanged); open FX then camera, press Esc twice (camera closes first, then FX). Verify `./scripts/live-check.sh winshell` is green and `./scripts/live-check.sh` (all) still green; screenshots listed here
- [ ] 4.2 `src/apps/kt-app/CLAUDE.md`: the rule "a window is made only by the shell; its kind decides Esc, cross, scroll and width" next to the R.66 form contract. Verify t921 names this rule in its failure message
- [ ] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
- [ ] 5.2 After archiving: append to `openspec/specs/ui-reach/design.md` the "why" (the hand-kept Esc list lost four windows; layer order instead of priority; the kinds table) and to `openspec/specs/ui-timeline/design.md` the growth mechanism found in 1.3
