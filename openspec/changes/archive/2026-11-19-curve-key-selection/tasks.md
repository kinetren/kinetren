# Tasks

Tests t3114–t3116 (the `features-slow` block). Branch `curve-key-selection` from `orch-26-11-01` (3fb5cb96).

## 1. The graph

- [x] 1.1 `SegSel` keys: segment ends + Shift-taken keys − taken-out ends; `key_on`, `key_indices`, `toggle_key`
- [x] 1.2 The joint drag (`KeyDrag`): time on the grid, value on the scale, clamped by neighbours and the axis; one history step; the selection follows
- [x] 1.3 Shift+click on a key, Shift sweep adds, a plain press elsewhere drops; selected keys drawn white
- [x] 1.4 A drag keeps the target and zone of its press point (D4)
- [x] 1.5 Key hover hint ×5 catalogs, `docs/guide/shortcuts.md`; t3114–t3116

## 2. Live, docs, close

- [x] 2.1 Live scenario `scripts/live/curve_keys.py`
- [x] 2.2 kt-app tests, clippy, rest cost
- [x] 2.3 What's new; registry A1 and U9; F-AUT-005; archive
