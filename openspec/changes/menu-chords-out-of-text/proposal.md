# Proposal

## Why

Five timeline menu labels carry their key inside the translated text:
`tl.menu.copy` "Copy (Ctrl+C)", `tl.menu.paste` "Paste (Ctrl+V)",
`tl.menu.remove_morpher` "Delete morpher (Del)", `tl.menu.delete_in_region`
"Delete clips in the region (Del)" and `tl.menu.region_from_event` "Region over this event
(Shift+double click)". That breaks `i18n/README.md` rule 3 (keys are glued on in Rust, in
the chord column). Two of the promises are also false where they stand:
- **Paste** is `paste_at_click`, which pastes at the click's time and row (R.56, owner's
  decision). `Ctrl+V` pastes at the playhead.
- **Delete morpher** deletes THIS morpher. `Del` deletes every selected morpher and camera,
  and a right-click on a member of a multi-selection keeps the selection.
Found while finishing `split-menu-label` (2026-10-26). The owner said to do it here and chose
the chord column, translated like "Enter, 2×click", for the gesture.

## What Changes

- Record the owner's decisions in `docs/user-feedback.md` before any other edit.
- All five labels lose their key in all five catalogs.
- A chord is shown only where the key does exactly what the item does:
  - Copy (event menu): `Ctrl+C`. The item selects the clicked event and then runs the same
    `copy_selection` the key runs.
  - Delete clips in the region (empty-space menu): `Del`. Same `delete_action`; the item is
    shown only with a region.
  - Delete morpher: `Del` only when that morpher is the whole selection (no other morpher, no
    camera). Otherwise no chord. Same pattern as the event menu's Delete `del_accel`.
  - Paste (event and empty-space menus): no chord, because the item pastes at the click.
  - Region over this event: the gesture in the chord column, a new catalog key
    `tl.menu.region_from_event_accel` ("Shift+2×click", translated like `nest_*_accel`).
- New regression test t871. No `[tl.menu]` or `[menu.*]` value in any language carries a key
  (the `*_accel` keys are chord texts by design). The morpher chord decision is a pure
  function, tested for both selections. The event and empty-space menus are probed in the
  harness for the chord each item shows.
- Behaviour does not change: every item runs exactly what it runs today.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `ui-timeline`: a new requirement. A menu label never carries its key; the chord column shows
  a chord only where the key does the same thing in that context.

## Impact

- `src/apps/kt-app/src/timeline.rs`: the five items drawn through `widgets::menu_item`; a pure
  `morpher_del_accel`.
- `src/apps/kt-app/i18n/{en,ru,es,zh-Hans,ar}.toml`: five labels shortened; one new key. No
  new characters: "2×", "клик", "clic", "双击" and "نقرة مزدوجة" are already in the catalogs.
- Tests: t871 in `src/apps/kt-app/src/reach_tests.rs`; the UI-text guards (`i18n_tests.rs`,
  `font_tests.rs`, `reach_tests.rs`, which matches the Delete-morpher item by "contains" so the
  chord does not break it, and `layout_tests.rs`).
