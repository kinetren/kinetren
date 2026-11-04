# Proposal

## Why

Two items of ktprompts 26.10.18 (recorded in `docs/user-feedback.md`, round "The owner's list
of 2026-10-27 and ktprompts v26.10.18"): "4. If I selected one event, copy-paste must take it,
not the region I selected. 5. A triple click on an event selects a region over its bounds."
Today Copy takes the region whenever one exists (`KtApp::copy_selection`,
`src/apps/kt-app/src/app.rs:4568`): a loop region kept for listening turns "click one event,
Ctrl+C" into "copy every event of every track in the loop, cropped". And the region over an
event exists only on Shift+double-click and in the menu — the gesture the owner reached for
first, a triple click, does nothing beyond the double click (it opens the FX window).

## What Changes

- **Copy precedence, decided explicitly**: exactly one selected event (with its link partners)
  → that event whole, the region is ignored and kept; otherwise a region → the region block, as
  today; otherwise → the selected events, as today. The status line names which of the three was
  taken. Paste is unchanged (it puts the clipboard at the playhead / the click).
- **Triple click on an event** sets the region over it through the same body as Shift+double-click
  and the menu item (`region_from_event`, the span of the event's link group). The side effect of
  the gesture's second click is undone: the FX window it opened closes, a reel it expanded
  collapses; the level line keeps its own double-click meaning (reset) and ignores the third click.
- The event hint and the menu item's chord column promise the triple click (all five catalogs);
  Shift+double-click keeps working.
- Docs: `docs/guide/shortcuts.md` (Ctrl+C row, a "triple-click an event" row) and
  `docs/guide/timeline.md` (the gesture table) describe the new rules.
- Feature note `docs/features/timeline/assembly/copy-target-and-region-over-event.md` (F-TL-013):
  four of five core editors let the selection win over the range in Copy.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-timeline`: the requirement "Multiple selection, marquee, moving a batch, double clicks" gains
  the triple click (one body with Shift+double-click and the menu) and the explicit copy
  precedence (one event → event; region → region; else selection), with the status-line naming.

## Impact

- Code (kt-app only): `copy_selection` in `src/apps/kt-app/src/app.rs`; the event click branch in
  `src/apps/kt-app/src/timeline.rs` (~3221–3275, `response.triple_clicked()` next to
  `double_clicked()`); catalogs `src/apps/kt-app/i18n/*.toml` (`hint.clip`, the
  `region_from_event_accel` chord, three copy notes); no model change (`kt_model::ops::copy_clips`
  and `copy_region_block` are reused as they are).
- Tests: `layout_tests.rs` t924–t928, `reach_tests.rs` t929; the existing t205, t206, t207, t250
  stay green.
- Delete keeps its region-first order in this change (design.md Open Question 1).
- No format, engine or invariant change.
