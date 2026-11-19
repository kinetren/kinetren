# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 156, MW1): "как-то все таки обыграть перенос на таймлайн
чтобы видео встало в нужную точку сразу". The timecode half was done (`bin-scrub-timecode`, F-MED-008);
asked how the drag should land, he answered (kkt-26.11.18b U9): "целиком, момент под мышью. а если
прожали alt то начало события будет под курсором, может быть еще какую-то комбинацию чтобы конец был
под курсором … до момента отпускания зажать альт … чтобы уже визуально до вставки была голограма события".

## What Changes

- A bin card pressed on its scrub strip carries the file moment it was pressed on; dropped on a track
  the WHOLE file lands with that moment under the mouse.
- Alt held over the track puts the event's start under the mouse, Alt+Shift its end; the modifier is
  read every frame, so pressing or releasing it mid-drag moves the landing before the drop.
- The drop ghost is the event: the file's (or the reel's) length at its real start, with a white tick
  at the point under the mouse; the status line says which point lands there and the other keys.
- A dragged card no longer writes its own hover text over the drop's line.

## Capabilities

- `ui-timeline`: ADDED requirement "A bin card lands with the scrubbed moment under the mouse".

## Impact

`kt-app` (`bin_panel.rs`, `timeline.rs`, `app.rs`), five catalogs, `docs/guide/shortcuts.md`,
`scripts/live/kt_live.py` (`drag(mods_mid=…, before_release=…)`). Tests t3117–t3119; live
`bin_drag_moment`. No model or format change.
