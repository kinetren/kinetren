# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 70, registry A1): "в таймлайне автоматизации нужна также
возможность выделения ключей и перемещения их". A curve key was dragged one at a time; the graph could
select SEGMENTS (segment-shape-fields) but not move anything together. Asked how a key selection should
relate to the segment one, he answered (kkt-26.11.18b U9, 2026-11-19): "п.1 а еще можно шифтом
прокликать ключи нужные или шифтом выделить еще несколько участков" — the keys of the selected
segments ARE the selected keys and move together; Shift+click on a key takes it in or out; Shift with
a sweep adds more segments.

## What Changes

- The automation graph's selection gains keys: the ends of every selected segment, plus keys taken in
  by Shift+click on a key (Shift+click on a selected key takes it out).
- Dragging any selected key moves the whole selection: the same time shift (the grabbed key snaps to
  the frame grid, the others keep their distances) and the same value shift on the graph's scale,
  each value held inside its parameter's range. The block cannot pass an unselected key or leave the
  visible axis. The drag is one history step.
- A sweep over empty space with Shift adds to the selection, as with Ctrl.
- A plain press on a key outside the selection drops the selection (as a click on empty space does).
- Selected keys are drawn filled white; the segment hover hint and the key hover hint say the gestures.

## Capabilities

- `automation`: ADDED requirement "The selected keys of a curve move together".
- `ui-reach`: MODIFIED requirement "Selecting segments is reachable by mouse and keyboard and is said"
  — the hint names Shift+click on a key and the joint drag.

## Impact

`kt-app` `automation.rs` only (one widget for every carrier: effect slot, generator, composition
parameter, camera, transition, mask), five catalogs, `docs/guide/shortcuts.md`. No model or format
change: the selection is window state. Tests t3114–t3116; live scenario `curve_keys`.
