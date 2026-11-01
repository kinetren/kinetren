# Proposal: windows-fill-their-area

## Why

The owner, kkt-26.11.01 item 12 (registry row K12, `docs/ktprompts/kkt-26.11.01.txt` line 22): an
enlarged Guide window "does not fill its area — you look through a needle's eye on a big window …
and so everywhere … I am tired of reminding"; the effects window "does not let itself grow past a
certain size"; "some windows grow in width but not in height"; the effect picker "splits into two
sections, and there too it is a needle's eye"; the Guide's tables have a narrow first column and
"look cheap"; the "I want to…" entries "run together, though they are separate sections". The
"repeated" table of `docs/user-feedback.md` names it ("a needle's eye", open). Each window was fixed
on its own before, so the defect came back in the next one: it needs ONE rule and a guard over every
window, not a fix of one window.

What causes it today (read in the code):

- `widgets::window_body` caps the body's scroll at 0.62× the SCREEN height and auto-shrinks it to
  its content, so an enlarged window grows an empty frame around a fixed body;
- `window_shell::Shell` makes a window resizable only when the site asks: twelve of the 24 windows
  are not resizable at all (track, search, settings, render, FFmpeg, welcome, dialogs);
- fixed sizes inside windows: the Guide's left column (260 px) and both its lists (0.55× screen),
  the search results (320 px), the render log (160 px), the picker's two scrolls (`GALLERY_W` 520 /
  `GALLERY_H` 450 / `PRESETS_H` 160) and the media picker (260 px);
- the Guide renders a table as a two-column `egui::Grid` of wrapping labels (the first column
  collapses to its narrowest word) and each "I want to…" entry as one frameless wrapping button;
- the bottom toolbar row needs ~1460 px and cuts off its right end (Render, Search, Diagnostics,
  Save) in a narrower window (found by the `tour-lessons-fixes` executor).

## What Changes

- **One window rule in `widgets.rs`/`window_shell.rs`:** every window of the registry is resizable;
  its body FILLS the window (the scroll takes the window's height minus the action row, and its
  width) instead of a screen fraction; the window's size is remembered per window (egui's persisted
  resize state for single windows, a per-family memory for windows opened per object — effects,
  morpher, transition, camera, track).
- **Fixed caps removed:** the Guide, search, render log, notices and every `window_body` grow with
  the window; a growing list or preview takes the room it is given.
- **The Guide:** tables become grid tables (header row, cell rules, stripes) with column widths from
  their content; "I want to…" entries become separate cards (what, then where); the left column takes
  a share of the window, the page the rest.
- **The FX picker:** one gallery in one scroll (presets as a cell row, then effects by category),
  as wide as the effects window it opens from and as tall as the screen below it allows; the cells
  flow into as many columns as fit. The media picker list takes the same screen-bounded height.
- **The bottom toolbar** no longer cuts off: the groups that do not fit move into a "more" (⋯) menu
  at the row's end — nothing is hidden without a way to reach it.
- **Guards:** a test that walks every window of the registry, enlarges it by dragging its corner and
  asserts that it grew and its body filled it (no fixed cap); a scan for fixed sizes in window code
  with every allowed one listed with a reason; a harness test for the Guide tables, the cards, the
  picker gallery and the toolbar overflow; a live scenario with screenshots of the main windows at a
  small and a large size.
- **Revision (ui-reach R.54):** `GALLERY_W` as a FIXED width of the picker is withdrawn — the rule
  "nothing jumps while typing" is kept by sizing the popup from the effects window, not from the
  content.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-reach`: the window form contract gains "the body fills an enlarged window" and "every window
  is resizable and remembers its size"; the Guide renders grid tables and index cards; the FX picker
  is one growing gallery.
- `ui-timeline`: the bottom toolbar row overflows into a menu instead of cutting off its tail.

## Impact

- Code: `src/apps/kt-app/src/{widgets.rs, window_shell.rs, guide.rs, fx_registry.rs, toolbar.rs}`
  and the window sites (`app.rs`, `fx_editor.rs`, `morpher_editor.rs`, `camera_editor.rs`,
  `transition_editor.rs`, `track_window.rs`, `render_dialog.rs`, `settings_window.rs`, `pubparam.rs`,
  `envclip.rs`, `ffmpeg_setup.rs`, `welcome.rs` — small hunks: the size call and the body wrapper).
- Tests: t1125–t1144 in a new `window_fill_tests.rs`; t779/t918/t923 must stay green (a resizable
  window must still not grow by itself).
- Live: `src/scripts/live/window_fill.py`.
- Docs: `docs/guide/whats-new.md` (kt26.11), registry K12, `docs/user-feedback.md` (taken as).
- No format, engine or dependency change.
