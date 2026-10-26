# Design

## Context

- **Three paths, one body.** The Edit menu item (`toolbar.rs:452`,
  `Self::menu_btn(ui, tr(k::menu::edit::CUT_REGION), "S")`), the event menu item
  (`timeline.rs:5891`, `ui.button(tr(k::tl::menu::SPLIT))`, which selects the clicked event
  if needed) and the `S` key (`handle_keys`, `app.rs:7138`) all call `KtApp::cut_action`
  (`app.rs:5021`). That function splits the selection with its link partners, or, with
  nothing selected, every event under the playhead. It never reads the region.
- **Two names for it.** `menu.edit.cut_region` reads "Split / cut region" (en; ru "Разрез /
  вырез региона", es, zh-Hans and ar the same). `tl.menu.split` reads "Split at playhead (S)",
  with the chord inside the translated text in all five catalogs. That breaks `i18n/README.md`
  rule 3 ("there are no hotkeys in the menu… glued together in Rust") and the `widgets.rs:725`
  comment ("the chord is an argument, not part of the translatable phrase").
- **The real cut-out** is `punch_out_action` behind **Cut out the region interior**
  (`tl.menu.punch_out`, `timeline.rs:1732`, the empty-space menu). It is untouched.
- **Owner's decision (2026-10-26):** rename the Edit item to "Split at playhead". `S` and the
  action stay as they are (item 5: "S: ALWAYS a cut at the cursor").

## Goals / Non-Goals

**Goals:**
- Every path to the split shows the same name, and the chord is shown the way menus show it.
- A test that fails if the Edit item's label or chord drifts from what `S` does.

**Non-Goals:**
- Any change of behaviour: `S`, `cut_action`, the punch-out item and the region all stay.
- A cut-out item in the Edit menu (offered to the owner and not chosen).
- Re-checking every other menu label for chords in text. If others are found, they are listed
  in the report, not fixed here.

## Decisions

### D1. One catalog key for one action
The Edit item uses `k::tl::menu::SPLIT`, the event menu's key. `menu.edit.cut_region` is
removed from all five catalogs. The build already refuses a key that exists in only some
catalogs, and t125 refuses an unused key, so a half-done removal is caught.
Rejected: a renamed second key (`menu.edit.split`) with the same text. Two strings for one
action would let the next translator make them differ, which is the drift that created this
defect.

### D2. The chord goes to the chord column in both menus
`tl.menu.split` loses " (S)" / "（S）" in every catalog. `timeline.rs:5891` draws the item
through `crate::widgets::menu_item(ui, tr(k::tl::menu::SPLIT), Some("S"))`, the same helper
the Delete item next to it uses. The chord is promised truthfully: after the item selects the
clicked event, `S` would split exactly that selection. No new characters enter any catalog, so
`scripts/subset-fonts.py` and t100 are unaffected.

### D3. t870 asserts the consequence from where the person acts
In `layout_tests.rs`, on `harness_three_clips_dt`, the test draws a region over part of event 1,
puts the playhead inside event 1 and clears the selection. It then:
1. clicks the Edit title, finds the item by its accessible label
   `format!("{}\t{}", tr(k::tl::menu::SPLIT), "S")` (the `menu_item` form), and clicks it;
2. records the event ranges. The test expects one more event, with both halves meeting at the
   playhead, an unchanged region, and an unchanged total duration (the interior was not cut out);
3. undoes, checks the original ranges are back, presses `S`, and asserts the identical ranges;
4. for each language in `Lang::ALL`, reads `tables::TABLES[lang][k::tl::menu::SPLIT]` (the
   way t171 reads the catalogs) and asserts it contains no chord: no `(S)`, `（S）` or
   standalone `S`. That `menu.edit.cut_region` is gone needs no assertion: the constant
   disappears, so any remaining use fails to compile, and a key left in only some catalogs
   fails the catalog build.
The test is written first and must fail on the current code: the Edit item is labelled
"Split / cut region", so step 1 finds no such button.

## Risks / Trade-offs

- **The Russian harness** (`harness_three_clips_dt` runs in `Lang::Ru`). The label is
  taken through `tr()`, never a literal, so the test is language-independent.
- **A live scenario that finds the Edit item by its old text.** Mitigated: `grep -rn
  "cut_region\|Split / cut" src/scripts/live` before the change. None found at planning
  time; the task re-checks.
- **The Arabic rendering of the chord column.** `widgets::menu_item` already serves the
  Delete item in the same menus, so no new layout case appears.
