# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 35, registry U9): "нужна функция cut(ножницы) региона и
чтобы еще с клавиатуры можно было (кнопка "C" свободна?)". The cut-out exists — "Cut out the region
interior" in the menu of a track's empty space (`punch_out_action`) — but only there: a person who
draws the region with I and O has to take the mouse and find an empty spot to right-click. Every
other editor puts the gap-keeping range cut on a key next to its range keys (Premiere Lift `;`,
Resolve Backspace on In/Out, FCP Shift+Delete on a range; feature note F-TL-015).

## What Changes

- A new command `Cmd::PunchOut` (37, append-only), "Cut out the region interior", in the Region group
  of the key list; in OUR layout its key is `C`, as he asked. The other layouts (Premiere, Resolve,
  Final Cut, After Effects) bind C to tools we do not have and leave it unbound; a person can bind
  it in Customize.
- Edit menu: "Cut out the region interior" with the key in the chord column; the empty-space item
  shows the same chord. One body for the three paths (`punch_out_action`, rule 17).
- Without a region the action says "Nothing to cut out: draw a region first" instead of doing
  nothing silently (rule 16).
- `docs/guide/shortcuts.md` gets the row.

## Capabilities

- `ui-controls`: ADDED requirement "The region's interior is cut out by a key and the Edit menu".

## Impact

`src/apps/kt-app/src/controls.rs`, `app.rs`, `toolbar.rs`, `timeline.rs`, five catalogs (two keys
each), `docs/guide/shortcuts.md`. Tests t3102, t3103; live scenario `region_cut_key`.
