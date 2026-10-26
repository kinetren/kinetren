# Proposal

## Why

Edit → **Split / cut region** (chord `S`, `src/apps/kt-app/src/toolbar.rs:452`) promises two
actions, but it calls `KtApp::cut_action`, which only ever splits at the playhead. It splits the
selected events and their link partners, or, with nothing selected, every event under the
playhead. That `S` never cuts out the region is the owner's earlier decision (item 5, recorded
at `app.rs:5021`). Cutting out the region's interior is a separate item, **Cut out the region
interior**, in the menu of a track's empty space. The label breaks UI rule 18 ("the name of a
menu item says WHAT and WHERE"). Found by the full shortcuts check (`guide-shortcuts-reference`,
2026-10-26). The owner chose: rename it to "Split at playhead", and keep the action and `S`.

## What Changes

- Record the owner's decision in `docs/user-feedback.md` before any other edit.
- The Edit menu item is labelled **Split at playhead**, with the `S` chord in the chord column.
  It uses the SAME catalog key as the event menu's split item (`tl.menu.split`), so the two
  paths to one action cannot drift apart by name again. The `menu.edit.cut_region` key is
  removed from all five catalogs.
- The event menu's split item stops carrying its chord inside the translated text
  ("Split at playhead (S)"). The chord moves to the chord column through
  `widgets::menu_item`, as `i18n/README.md` rule 3 requires. All five catalogs lose the "(S)".
- New regression test t870. The Edit item and the `S` key split the same events in the same
  place, even with a region drawn, and the region's interior stays. The item's visible
  label is the event menu's label, with `S` as its chord. No catalog keeps a chord inside the
  split label.
- Behaviour does not change: `S`, `cut_action` and the punch-out item stay as they are.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `ui-timeline`: the requirement "An impossible action is an error; paired ways are
  identical; names say what and where" gains a scenario. The split item in the Edit menu,
  the one in the event menu and the `S` key share one name and one body.

## Impact

- `src/apps/kt-app/src/toolbar.rs` (the Edit item), `src/apps/kt-app/src/timeline.rs:5891`
  (the event item through `widgets::menu_item`).
- `src/apps/kt-app/i18n/{en,ru,es,zh-Hans,ar}.toml`: `menu.edit.cut_region` removed,
  `tl.menu.split` without "(S)". No new characters, so the font subsets do not change.
- Tests: new t870 in `src/apps/kt-app/src/layout_tests.rs`; the mandatory kt-app guards for
  UI text (`i18n_tests.rs` t125/t128, `font_tests.rs` t100, `reach_tests.rs`, `layout_tests.rs`).
- Docs: `docs/guide/shortcuts.md` already says what `S` does. Nothing there names the
  Edit item, so it does not change.
