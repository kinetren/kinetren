# Proposal

## Why

Two complaints of the 2026-10-27 round (`docs/user-feedback.md`, "The owner's list of 2026-10-27
and ktprompts v26.10.18") come from the same cause: each of the 23 windows of kt-app builds its
own `egui::Window` by hand, so each one behaves a little differently. On the owner's other
system the **morpher window keeps getting wider** — fast while the mouse moves, slowly when it
does not (kkt item 2). The **camera window does not close on Esc** (ktprompts 26.10.18 item 6):
the Esc cascade (`escape_action`) is a hand-kept list of windows, and the camera, transition,
project settings and envelope-fit windows are not in it. The owner's words: "we have lots of
duplicated code, though I asked that all widgets and windows reuse code and have the widest
behaviour". The R.66 form contract (body scroll, buttons outside it) is held by t779, but
nothing holds the other things a window does — Esc, the title cross, scroll policy, width —
so they drift one window at a time. The morpher window still keeps the outer `vscroll(true)`
that the FX and transition windows removed in R.67.

## What Changes

- **One window shell.** Every window of the program (all `WINDOWS` registry entries, both
  `egui::Window` and `egui::Modal`) is built through one shell. Each window declares its kind
  (tool window, dialog, progress, message) and the shell sets everything a window does: the id,
  the title cross, collapsing, resizing, the scroll policy (one body scroll, never a window-level
  scroll), a stable width, and what Esc means for it.
- **Esc closes every window, top window first.** Esc closes the window the person sees on top
  (by egui's layer order), not the first match of a hand-kept list. What Esc does is declared per
  window: a tool window closes and keeps its edits (as the FX window does today), a dialog takes
  the action that loses nothing (Cancel, Later, No). A window that Esc must not touch (export and
  analysis progress: a minutes-long job must not die from a key meant for something else) says
  so with a written reason.
- **The title cross and Esc are one body.** The cross and Esc take the same path for every
  window (ui-reach rule "paired ways — one body").
- **A window's width does not grow by itself.** Its width changes only by the person's
  resize gesture or by content that really got wider, never by repeated frames. The first stage
  finds the morpher window's growth mechanism in the real window before the fix (a minimal egui
  0.36.1 reproduction of the assumed cause did not grow, see design.md).
- **Guards.** A scanning guard fails on any window built outside the shell and on any
  window-level scroll; a harness test checks width stability of every window at display scales
  1.0, 1.25 and 1.5; a harness test presses Esc on every window; the t779 table gets real
  openers for the morpher, transition and camera windows instead of three written-off entries.
- Inventory of the deviations the shell removes: design.md, "Window inventory".

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-reach`: the requirement "Windows only from the registry; … one widget" gains the shell:
  every registry window is built by one shell, which owns Esc, the cross, scroll and width; Esc
  closes every window of the registry, top first, with written exceptions.
- `ui-timeline`: a window does not grow by itself — width is stable over frames at fractional
  display scales (the defect "the morpher window grows wider").

## Impact

- Code (kt-app only): a new shell module next to `src/apps/kt-app/src/widgets.rs`
  (`window_body`/`window_actions` move into it or are called by it); every window site:
  `fx_editor.rs`, `morpher_editor.rs`, `transition_editor.rs`, `camera_editor.rs`,
  `track_window.rs`, `settings_window.rs`, `envclip.rs`, `render_dialog.rs` (2),
  `ffmpeg_setup.rs`, `pubparam.rs` (3), `app.rs` (analysis, notices, search, reel name, paste
  prefix, preset name, diagnostics, recovery and ask modals); `escape_action` in `app.rs`
  becomes "close the top shell window" plus its non-window steps.
- Tests: `reach_tests.rs` (t757 scanner follows the shell, t779 openers, new t919–t922),
  `layout_tests.rs` (t918, t923); a live scenario under a 1.25 scale factor.
- No model, engine, format or catalog change; no new dependency. Invariant 12 is untouched (the
  shell adds no repaint).
- Docs: `docs/guide/shortcuts.md` Esc row stays true and gains nothing new; `src/apps/kt-app/CLAUDE.md`
  gets the rule "a window is made by the shell".
