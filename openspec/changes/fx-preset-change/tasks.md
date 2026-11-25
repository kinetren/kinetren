# Tasks: fx-preset-change

> Decided (A4); revision 3 implementation-ready at `229c694b` (2026-11-24), graph v2d merged. Code starts on the orchestrator's go.
> Tests: t3595–t3596, t3932–t3934, t4243–t4244.

## 0. Notes

- [x] 0.1 F-UI-010; proposal, design (D0–D6), deltas `ui-reach`, `automation`; `openspec validate fx-preset-change --strict`
- [x] 0.2 his decision (kkt-26.11.20 lines 7–8) folded in

## 1. The preset of an effect (`fx_registry.rs`, `widgets.rs`, the three windows)

- [ ] 1.1 `presets_of`, the Preset menu at the form's head, `apply_preset` per owner, the reading; i18n ×5; t3595, t3596, t4243

## 2. Several nodes (`node_editor.rs`, `widgets::ChainEdit`)

- [ ] 2.1 `group`, Ctrl+click, the frame, Esc, drawing; `Op::RemoveMany`, `Op::ShiftMany`, renumbering; t3933, t4244
- [ ] 2.2 what `Op::Add`/`Op::Branch` inserted becomes the group, preset and paste; t3932, t3934
- [ ] 2.3 `docs/guide/shortcuts.md` (the graph's table: Ctrl+click, the frame, Delete of a group); live `node_graph_branch` gets the
      group steps, live `fxpreset` the Preset menu, a screenshot per step

## 3. Close

- [ ] 3.1 What's new ("an effect's preset is changed in place; several nodes are selected, moved and deleted with the mouse");
      F-UI-010 verdict; E6 and A4 moved; `finish.sh` (orchestrator)
