# Proposal

## Why

The owner answered Q1 (`docs/ktprompts/kkt-26.11.20.txt` line 4, row A1): "Один пункт, по дорожке
(Recommended)". After generator-from-track a track's right-click menu had one "Create a generator…"
item, while Edit and the bin still listed every kind in groups. Now they get the one item too. The kind
goes by the selected track: a video track gives a solid colour, an audio track a tone. With nothing
selected the item gives a solid colour.

## What Changes

- Edit's and the bin's "Create a generator…" become one plain item. It has the same body as the track
  menu's item (`gen_ui::generator_item`). The new event lands at the cursor on the track of the earliest
  selected event; with nothing selected it lands on video track 1. Its window opens, and the window's
  Kind gallery changes the kind.
- That makes the gallery the one way to any kind. Changing the kind there, or from the bin card, now
  keeps the generator's sound pair in step. A kind that sounds beside its picture (Sync flash) gets its
  sound event on A1, linked to the picture, as the menu gave before. A kind with no sound loses the
  former sound event. Each change is one history step. Before this, a Sync flash picked in the window
  had a silent beep, and a solid colour switched from a Sync flash kept an event of silence.
- Lesson 1, step 1 reads "Edit › Create a generator…" and then "in its window pick Text in Kind". Its
  outline moves to the window's Kind field.
- Twelve live scenarios reach a kind through `Live.new_generator` / `pick_generator_kind`.

## Capabilities

- `ui-reach`: ADDED requirement "Edit and the bin create a generator by the selected track".
- `generators`, `ui-reach`: the existing scenarios of t560, t1164, t3702, t3705, t3708, t3710 and the
  tour say the new path. Their tests follow the owner's decision.

## Impact

`apps/kt-app`: `gen_ui.rs`, `toolbar.rs`, `bin_panel.rs`, `app.rs`, `fx_editor.rs`, `lessons.rs`, the five
catalogs. Live scenarios: `scripts/live/kt_live.py` and twelve scenarios. No model or format change.
