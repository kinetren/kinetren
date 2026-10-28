# Design

## Context

See proposal.md — Why. Observed in the code (2026-10-28):

- `KtApp::copy_selection` (`src/apps/kt-app/src/app.rs:4568-4603`): `match region { Some(r) =>
  copy_region_block(project, r), None => copy_clips(project, selected_with_groups()) }`, then
  `localize` to the owning reel and the note `CLIPS_COPIED` with the count. The region wins with
  no look at the selection.
- `kt_model::ops::copy_region_block` (`src/libs/kt-model/src/ops.rs:319`) takes EVERY clip that
  crosses the region on every track, crops it by `clip_slice`, rebases to the region start;
  `copy_clips` (`ops.rs:369`) takes the ids with their whole link groups, uncropped. Both end in
  `block_extras` — the single rule of what travels with a selection (kt-model CLAUDE.md, "What
  travels with a selection — ONE RULE"). This change does not touch either.
- `delete_action` (`app.rs:5089`): selected morphers/cameras first, then "region → delete every
  crossing event", then the selection — the same region-first order. `docs/guide/shortcuts.md`
  states both orders today.
- `region_from_event` (`app.rs:4031`) — the span of the link group; one body for Shift+double-click
  (`src/apps/kt-app/src/timeline.rs:3243-3244`) and the menu item (`timeline.rs:5924-5927`),
  ui-timeline design "Double clicks: one body `region_from_event`".
- `region_from_marked_range` (`app.rs:4415`) sets the region AND selects every event crossing it
  (R.33 item 8: "on a double click on a marked_region it must be selected entirely").
- The click branch on an event (`timeline.rs:3221-3275`): `clicked()` → `select_only` + seek;
  `double_clicked()` → Shift: region; Level zone: reset gain/opacity (`SetGain`/`SetOpacity`, a
  history step); a reel: `expand_in_place`; otherwise `open_fx_editor`. egui 0.36.1 has
  `Response::triple_clicked()` (`egui-0.36.1/src/response.rs:242`); a triple click arrives on the
  third press, AFTER `double_clicked()` fired on the second (max delay 0.3 s,
  `input_state/mod.rs:117`). Nothing in kt-app reads `triple_clicked` today.

## Goals / Non-Goals

**Goals:**
- One explicit, testable precedence for Copy, named aloud in the status line.
- A triple click that leaves exactly "region over the event", nothing else.

**Non-Goals:**
- Paste placement (R.56 item 450: playhead + row under the mouse / the click) — unchanged.
- Delete precedence — unchanged here (Open Question 1).
- Copying morpher layers or cameras by their own selection — not a Ctrl+C target today, stays so.
- A "selection limited by the region" mode (Vegas): rejected below.

## Decisions

### D1. Precedence: one event → event; else region → region; else selection

The resolver lives in `copy_selection` only:

1. the selection, expanded by link groups (`selected_with_groups`), is exactly ONE link group (or
   one event without a group) → `copy_clips(that)`; the region stays as it is;
2. else a region exists → `copy_region_block` (today's behaviour);
3. else → `copy_clips(selected_with_groups)` (today's behaviour).

"One event" counts link groups, not ids: clicking a video event selects only it, but its audio
partner travels anyway, and a person who clicked one A/V pair has selected "one event".

Why not "any selection wins" (Premiere/Resolve, F-TL-013): it would silently change the
range-marker gesture of R.33 item 8, which selects the crossing events AND sets the region — its
Ctrl+C would stop cropping to the marker, and markers inside would travel by the selection's span
instead of the marker's (R.35 item 4: "marked_region also together with those inside it"). The
owner asked for the one-event case; several selected events keep today's rule until he decides
(Open Question 2). Why not Vegas's "selected events limited by the region": it needs a third block
builder (selected ∩ region, cropped), and it reverses the owner's request — the one event would be
cropped to the region.

Consequence written down, not hidden: a range marker that crosses exactly ONE event, double-clicked,
then Ctrl+C, copies that event whole (rule 1), not the cropped piece. The status line says
"the selected event", so the person sees which rule fired (t926). Alternative — remembering whether
the selection was made by a click or by a region gesture — rejected: an invisible state that
decides what Copy takes is a mode, and modes are what the owner keeps complaining about.

### D2. The status line names the target

Three catalog notes replace the one `msg.clips_copied`: "Copied the selected event: {0}",
"Copied the region: {0}", "Copied the selected events: {0}" (rule "noun: N", no plural forms).
Why: the precedence has three branches, and rule 16 of the checklist ("an action says what it
did") is the only way the person can tell them apart without pasting.

### D3. Triple click undoes its second click's view effect

In the event branch, after the `double_clicked()` block: `if response.triple_clicked()` on the same
event and NOT on the Level zone → `region_from_event(id)`; then, if the double click of THIS gesture
opened the FX window (the window was not open before the second click — remembered in a one-frame
field `pending_double: Option<(ClipId, DoubleEffect)>` set by the double-click branch), close it;
if it expanded a reel in place, collapse it. The Level zone keeps its double-click reset and ignores
the third click: undoing an edit by a click would put a history step in and out, and a triple click
on a 4-px line is not a plausible intent. Alternative — delaying the double click until 0.3 s pass
without a third click — rejected: every FX window would open 300 ms late, and a double click is
the gesture used a hundred times more often.

### D4. Hints and docs promise it

`hint.clip` in five catalogs gets "triple click — region"; `tl.menu.region_from_event_accel`
becomes the triple click ("3×click" and its translations), because the chord column shows the
simplest way; Shift+double-click keeps working and stays in `docs/guide/shortcuts.md`. The
catalog changes go through t125/t100 (glyphs, placeholders) and t128 (label budgets).

## Risks / Trade-offs

- [The FX window flashes for ~0.3 s during a triple click] → accepted; it is the cost of not
  delaying every double click (D3); t927 asserts it is closed after the third click.
- [Rule 1 surprises a person who selected one event and wanted the region] → the status line names
  "the selected event". Esc is not the way out (its step 3 clears the region AND the selection
  together); the way is Ctrl+click on the event, which deselects it and keeps the region, then
  Ctrl+C. The guide row states this, and t925 covers "region, nothing selected".
- [A triple click on a reel collapses a reel that was ALREADY expanded before the gesture] → the
  undo is keyed on what THIS gesture's second click did (it toggled), so collapsing back restores the
  state before the gesture in both directions.

## Migration Plan

No data or format change. Rollback is a revert.

## Open Questions

1. Should Delete follow the same precedence (one selected event + a region → delete only that
   event)? Recommended answer: yes — "what Ctrl+C takes is what Delete removes" is the ui-reach rule
   "paired ways — one body"; it is left out of this change only because the owner asked about
   copy-paste, and Delete removes material. If he says yes, it is one more stage with its own test.
2. With SEVERAL selected events and a region, should the selection win (as Premiere and Resolve
   do)? Recommended answer: no for now — it changes the range-marker double-click copy (D1); revisit
   if the owner meets the case.
