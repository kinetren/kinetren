# Tasks

Tests t3102–t3103 (the `features-slow` block). Branch `region-cut-key` from `orch-26-11-01`.

## 1. Note, command, doors

- [x] 1.1 Feature note `docs/features/timeline/assembly/cut-out-region-key.md` (F-TL-015)
- [x] 1.2 `Cmd::PunchOut = 37` in `COMMANDS` (Region), `CMD_COUNT` 38, `OWN` binds `C`; `handle_keys` → `punch_out_action`
- [x] 1.3 Edit menu item with the chord (probe `menu.edit.punch_out`); the empty-space item prints the chord; no region → `msg.punch_out_no_region`
- [x] 1.4 Catalogs: `controls.cmd.punch_out`, `msg.punch_out_no_region` in five languages; `docs/guide/shortcuts.md` row
- [x] 1.5 t3102, t3103 green; red on the old code (no command: `C` did nothing — "C left material inside the region")

## 2. Live, docs, close

- [x] 2.1 Live scenario `scripts/live/region_cut_key.py`, five screenshots; green first try
- [x] 2.2 i18n/font/reach/layout/controls tests 422/422 green; rest cost: no draw path at rest changed (two menu items, drawn only while their menu is open)
- [x] 2.3 What's new; registry U9 → done; F-TL-015 done; archive
