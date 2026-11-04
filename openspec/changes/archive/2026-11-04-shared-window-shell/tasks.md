# Tasks

Test numbers t918–t923 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files: the checkout
may hold another session's uncommitted edits.

## 0. Records before code

- [x] 0.1 The owner's complaints (kkt item 2, ktprompts 26.10.18 item 6) are recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this proposal names `shared-window-shell`. Verify `openspec validate shared-window-shell --strict` is green

## 1. Find the growth first (investigation, no fix)

- [x] 1.1 Write t923 in `src/apps/kt-app/src/layout_tests.rs` with the owner's morpher scene (batch layer, 12-slot strip, two area references, an open curve graph, a source field) at `with_pixels_per_point(1.25)` and `1.5`, 600 frames, the pointer moving over the main window and over the morpher window, logging the width each 50 frames. Verify: record here whether it is RED on today's code and the per-50-frame widths
  - RED, but only once the harness has the PROGRAM's style: the first version used egui's default
    margins (the harness never called `theme::apply`) and grew at 1.2 while the real program did
    not. `harness_scaled` now applies the theme as `main` does. With it the morpher window grows by
    one physical pixel PER FRAME at zoom 1.1, 1.3, 1.35, 1.85 (a sweep 1.05…2.0 on 1440/1600/1920
    screens gave the same four), not at 1.2/1.25/1.5. Zoom 1.1, every 50 frames: 564.56 610.91
    656.38 701.81 747.28 792.72 838.19 883.62 929.09 974.56 1020.00 1065.47 → 1110.00 (pointer
    over the main window, over the morpher window, or still — the same: it is the frame count,
    and the owner's "slowly when the mouse does not move" is the program's idle repaint rate).
    Scale via the program's zoom (`set_zoom_factor`) and via native scale behave the same.
- [x] 1.2 Run the real program under Xvfb at the program's own zoom 1.1 and 1.2 (Ctrl+= pressed once and twice — the owner's case, 2026-10-28: Ubuntu, 1920×1080, native scale 1.0), and with `WINIT_X11_SCALE_FACTOR=1.25` as a second case (new live scenario `winshell` in `src/scripts/live/`, which the specs name once it exists; `kt_live.step` screenshots at 0 s, 10 s and 30 s of pointer motion), measuring the morpher window's width through `scripts/ui-probe.py`. Verify: record the widths here; if neither 1.1 nor 1.2 grows, stop and ask the owner design.md Open Question 1 before stage 4
  - `src/scripts/live/winshell.py` (the morpher layer with twelve slots, the first slot selected
    with its curve graph open). Ctrl+= ONCE (zoom 1.1, the dump confirms 1440 → 1309.1 points):
    the window widened 608.2 → 663.6 → 698.2 points in fifteen seconds of mouse motion, 99
    frames. Ctrl+= twice (1.2): no further growth. So the owner's "a couple of times" passed
    through 1.1, where it ratchets. `WINIT_X11_SCALE_FACTOR` was not needed: the harness
    sweep shows native scale and zoom are the same mechanism.
- [x] 1.3 Bisect in t923 (comment the parts out in a scratch copy, not in the commit): the window `vscroll`, then each group of the body (targets, source, chain strip, refs, curve block, thumbnail picker). Verify: the widget or option that feeds the width back is named here with its `file:line`, and design.md D3 gets the chosen variant (a/b/c) as a dated line
  - The feeding option is the window-level scroll `.vscroll(true)` at
    `src/apps/kt-app/src/morpher_editor.rs:330` (and the same at `camera_editor.rs:84`): with it
    commented out, the sweep 1.05…2.0 grows at 0 of 20 zooms on all three screens; with it, at 4.
    No widget of the body had to be removed. Variant (a) of D3.

## 2. Tests first

- [x] 2.1 Write t918 in `src/apps/kt-app/src/layout_tests.rs`: every openable registry window at scales 1.0/1.25/1.5, 300 frames with pointer motion, outer rect within one physical pixel; reuse the openers of t779. Verify it compiles and record which windows are RED today
  - Scales 1.0, 1.1, 1.2, 1.25, 1.5 (1.1 added: the owner's first Ctrl+=). RED today: `morph`
    and `camera` at 1.1 (565 → 839, 561 → 834: the two windows with `vscroll(true)` and a wide
    body); `track_window` at every scale grows 451 → 629 in height over its first six frames
    (30 points a frame up to a ceiling — its own copy of the body scroll, `track_window.rs:139`);
    `project_settings` +2 points in width at every scale. `kt_notices` and `kt_diag` keep
    `vscroll(true)` but their bodies are narrow and did not grow.
- [x] 2.2 Give t779 real openers for `morph`, `transition`, `camera` (scenes of t242, t227, t231b) in `src/apps/kt-app/src/reach_tests.rs`, removing their `OpenWin::Cant`. Verify t779 runs them and record whether the form contract holds for them today
  - The openers are `layout_tests::{owner_morpher_scene, transition_scene, camera_scene}` (the
    scenes call the same `open_*_editor` the double click calls). t779 is green with them: the
    form contract (actions outside the scroll, reachable in a low window) holds for all three.
- [x] 2.3 Write t919, t920, t922 in `reach_tests.rs` with the `- **PATH**` of the spec. Verify: t919 is RED on `camera`, `transition`, `project_settings`, `env_fit`, `pub_unpublish` and the two modals; t920 is RED (fixed order closes FX before camera); t799 accepts the PATH lines
  - RED on the old code. t919: `project_settings`, `kt_recovery`, `kt_ask`, `transition`, `camera`
    stay open after Esc, plus the owner's camera case (`env_fit` and `pub_unpublish` cannot be
    opened by the harness — their scenes are other tests'; `paste_names` and `preset_save` got
    openers from their own state). t920: "the first Esc must close the camera window — it is on
    top". t922: a click on the title's left end folds `fx`, `kt_notices`, `kt_diag`, `morph`,
    `transition`, `camera` (the collapse arrow; it needs four frames of animation to show), and
    Esc does not close `project_settings`, `transition`, `camera`.
- [x] 2.4 Write t921 in `reach_tests.rs` (scanner outside the shell module). Verify it is RED and lists the 23 sites and the four `vscroll(true)` calls of the inventory
  - RED: the shell module did not exist. It scans with comments stripped (`eq_graph.rs` mentions
    `.vscroll(true)` in a comment).

## 3. The shell

- [x] 3.1 Create the shell module (design D1): `Spec`, the closed `Kind` enum, `show` that sets id, cross, collapse, resize, the body scroll (`window_body` moved in) and the actions row, and returns open/closed-by/`LayerId`; the per-frame list of shown windows on `KtApp` (D2). Verify `cargo build -p kt-app` and `cargo clippy -p kt-app --all-targets` with no new warnings
  - `src/apps/kt-app/src/window_shell.rs`. Deviation from D1, on purpose: `window_body` and
    `window_actions` stay in `widgets.rs` and the site calls them inside the shell's body — the
    short dialogs (one field and a row of buttons, t779's `ActionShort`) have no body scroll, and
    wrapping every body would change their layout. The shell owns everything else. The list of
    shown windows lives in egui's frame data, not on `KtApp`: the sites are closures over
    `&mut self`. No new clippy warnings (the remaining ones are older than this change).
- [x] 3.2 Move the tool windows (`fx`, `morph`, `transition`, `camera`, `track_window` — dropping its own scroll copy, `published_win`, `kt_notices`, `kt_diag`, `kt_search`, `kt_ffmpeg`) onto the shell; the four `vscroll(true)` calls disappear. Verify t918 and t923 are green, t779 and t780 still green
  - The notices window had no body scroll of its own; with the window scroll gone it is a plain
    list in a resizable window. The track window lost its own scroll copy and now uses
    `window_body`: the copy grew the window 30 points a frame for six frames after opening.
    The camera window publishes its frame as `win.camera` (`Shell::probe`) for live scenarios.
- [x] 3.3 Move the dialogs and progress windows (`env_fit`, `project_settings`, `kt_render_settings`, `kt_render_progress`, `pub_name`, `pub_unpublish`, `seq_name`, `paste_names`, `preset_save`, `kt_analysis`) and the two modals onto the shell, with their declared Esc action; delete the four per-dialog Esc handlers. Verify t250, t343, t345 and the publishing tests of `compositions` still green
  - Esc of a modal is its safe answer: recovery → Later, the question → Cancel (`SaveOverLock`
    treats Cancel as No). The search window's own Esc went too. All 387 kt-app tests green.
- [x] 3.4 Replace the window part of `escape_action` by "close the top shell window" (D2); keep step 0 and the non-window steps. Verify t919, t920, t922 and the Esc cascade tests (t636 and the `layout_tests.rs` cases calling `escape_action`) are green
  - egui drops a text field's focus on Esc at the START of the frame (`Memory::begin_pass`), so
    `handle_keys` sees the key even from a dialog's field: the four per-dialog handlers were
    duplicates of the cascade, not a second route it needed.
- [x] 3.5 Apply the width rule of D3 chosen in 1.3 inside the shell. Verify t918 and t923 green at all three scales
  - Variant (a) only: no window scroll. One more grower showed up in t918 after the move — the
    project settings window widened two points on the first hover of an unselected curve-shape
    button (egui adds a frameless button's frame stroke to its size while hovered); the row now
    draws framed buttons. t918 runs scales 1.0/1.1/1.2/1.25/1.5, t923 zooms 1.1/1.2/1.25/1.3/1.5.
- [x] 3.6 Update t757's scanner to the shell call; verify t757 and t921 are green and the `WINDOWS` registry is unchanged byte for byte
  - The scanner looks for `window_shell::Shell::new(`/`window_shell::modal(` and skips the shell
    itself; the registry is untouched.
- [x] 3.7 Run the UI guards of `ui-reach` and `ui-timeline` (`reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`, `i18n_tests.rs`), then commit stages 1–3 (`cargo test --workspace --no-fail-fast` green)

## 4. Live and records

- [x] 4.1 Finish the `winshell` live scenario: at zoom 1.2 open the morpher window and move the pointer for 30 s (width unchanged); open FX then camera, press Esc twice (camera closes first, then FX). Verify `./scripts/live-check.sh winshell` is green and `./scripts/live-check.sh` (all) still green; screenshots listed here
  - `winshell` green on the shell build: zoom 1.1 → 560.0 560.0 560.0 points over fifteen seconds
    (it was 608.2 → 698.2 before), zoom 1.2 → 560.0 throughout; FX then camera, Esc closes the
    camera window first, the second Esc the FX window. Screenshots `target/live-out/winshell/`:
    01-step-zoom-1.1, 02-step-zoom-1.1-after-15s, 03-step-zoom-1.2, 04-step-zoom-1.2-after-15s,
    05-step-fx-and-camera, 06-step-esc-camera-gone, 07-step-esc-fx-gone.
  - The full run: green, with `formula`, `fxwin`, `nestshare`, `readme_shots`, `recover` passing on
    the alone re-run (red only in the parallel run under load: a click or a double click that did
    not land — the same pattern as `trans` in the previous change).
- [x] 4.2 `src/apps/kt-app/CLAUDE.md`: the rule "a window is made only by the shell; its kind decides Esc, cross, scroll and width" next to the R.66 form contract. Verify t921 names this rule in its failure message
  - Added next to "the window title ≠ its id" (the kt-app CLAUDE.md has no R.66 paragraph of its
    own), with a second rule found on the way: a frameless button in a row that sets a window's
    width grows it on hover.
- [x] 4.3 Commit stage 4

## 5. Finish

- [x] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
  - `./scripts/finish.sh`: **kt26.10.28-fc7a6977**. Live scenarios green (49, `nestparam` on the alone
    re-run); SIMD and the guards green. The workshop test block red on four load-timed tests —
    t815 (a 3.7 s write), t802 (one worker on a busy machine shares the display), t476, t411 — all four
    green alone (`--test-threads=1`) right after; none is on a path this change touches.
    Live screenshots: `target/live-out/winshell/01…07` (listed in 4.1).
- [x] 5.2 After archiving: append to `openspec/specs/ui-reach/design.md` the "why" (the hand-kept Esc list lost four windows; layer order instead of priority; the kinds table) and to `openspec/specs/ui-timeline/design.md` the growth mechanism found in 1.3
  - Done in the archive commit (2026-11-04): `openspec/specs/ui-reach/design.md` "One shell for every window" and `openspec/specs/ui-timeline/design.md` "A window that grew by itself".
