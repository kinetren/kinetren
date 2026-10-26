# Proposal

## Why

The site (kinetren.github.io) dropped the "Keyboard" section from its landing page, so
`docs/guide/shortcuts.md` is now the only key reference, and the site shows it as
`/docs/shortcuts/`. The page lists less than the landing did. It has no region or view
groups, writes keys as bold text, and leaves out gestures the program handles (double-click on
the preview, the timecode, the speed strip and the fader; Ctrl+click on an event; the
track-header gestures). While mapping the keys against the code, one defect turned up:
`Alt+S` (solo) also splits the event under the playhead, because the split check
(`src/apps/kt-app/src/app.rs:7136`, `S && !ctrl`) does not exclude Alt.

## What Changes

- Record the owner's request (2026-10-26) in `docs/user-feedback.md` before any other edit.
- Build an inventory of every key and mouse gesture the program handles, taken from the
  code: the hotkey block `KtApp::handle_keys` (`app.rs`), the timeline (`timeline.rs`),
  `bin_panel.rs`, `automation.rs`, `pubparam.rs`, the toolbar timecode mask (`toolbar.rs`),
  menu chords (`Toolbar::menu_btn`, `widgets::menu_item`) and the hint lines in
  `i18n/en.toml`. Check each row of the former landing list against that inventory and
  list every difference for the report.
- Rewrite `docs/guide/shortcuts.md` as the complete reference. Group the tables by what a
  person looks up together (Transport · Editing · Region and markers · Reels · View and
  zoom · Mouse on the timeline · Windows and panels). Every table is `| Keys | Action |`,
  every key a `<kbd>` cap. A key is described only where it really does that action. The
  file name does not change.
- Fix the text of `timeline.md` / `getting-started.md` wherever it contradicts the new
  tables. Cross-page links stay `page.md#anchor`.
- Fix `Alt+S`: the split ignores the Alt chord, so `Alt+S` only toggles solo. Regression
  test: `Alt+S` with the playhead over the selected event leaves the event count unchanged.
- Add a `markup` class to `src/scripts/readme-check.py` for `docs/guide/**`. It allows only
  the Markdown subset the site renders (no HTML except `<kbd>`, no `>` quotes, no setext
  headings, no reference links or footnotes, no `_italic_`, no backslash except `\|`, at
  most one level of list nesting). For `shortcuts.md` it also requires `| Keys | Action |`
  table headers and allows no bold key chords. `cargo test` and `finish.sh` already run
  the guard.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `showcase`: a new requirement. The guide stays within the site's Markdown subset, and
  `shortcuts.md` is the one complete key reference, written with `<kbd>` caps in
  `| Keys | Action |` tables. Both are guarded by `readme-check.py`.
- `ui-timeline`: a new requirement. A chord with Alt or Ctrl never also fires the action
  of its bare letter (the `Alt+S` → split defect).

## Impact

- Docs: `docs/guide/shortcuts.md` (rewritten), `docs/guide/timeline.md` and
  `docs/guide/getting-started.md` (only where they contradict), `docs/user-feedback.md`.
- Code: `src/apps/kt-app/src/app.rs` (the `cut` condition in `handle_keys`),
  `src/apps/kt-app/src/layout_tests.rs` (a new test),
  `src/scripts/readme-check.py` (the `markup` class),
  `src/apps/kt-cli/tests/readme_guard.rs` (a new test).
- No UI text, layout, catalog or format change. No new dependency.
- Site: after the commit the owner says "обнови" in kt.9.site, and `upstream-sync`
  pulls the new page in. This repository does not touch the site.
