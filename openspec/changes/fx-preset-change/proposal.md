# Proposal: fx-preset-change

> DESIGN ONLY (spec session, 2026-11-20), revision 2 after the owner's answer (`kkt-26.11.20` A4, his Q52). Code
> starts on the orchestrator's go. Tests reserved: t3595–t3596, t3932–t3934.

## Why

The owner, `kkt-26.11.07` line 12: "невозможно после добавления эффекта - ПОМЕНЯТЬ ЕГО ПРЕСЕТ НЕ ПЕРЕСОЗДАВАЯ
ЕГО НУЖНО ПОСТОЯННО ЕГО УДАЛЯТЬ ЕСЛИ ПЕРЕДУМАЛ И ЗАДАВАТЬ ЗАНОВО ИЩЯ УЖЕ ПРЕСЕТ В ВЫПАДАЮЩЕМ МЕНЮ." Today the
kind's variants (`UiDecl.variants`) are offered only by the picker when a slot is ADDED, and a chain preset is
appended at the end of the chain. Note F-UI-010.

His decision, 2026-11-20 (`kkt-26.11.20.txt` lines 4–5): "если это пресет конкретного эффекта - то заменяется. если
это прессет цепочки - то можно просто к существующей цепочке на графе соединить ту что с пресета или мышкой быстро
выделить и удалить лишнее или соединить".

## What Changes

- **A "Preset" menu at the head of every effect slot's form** (above the rows), listing the same variants the
  picker offers for this kind, then the user's single-effect presets of this kind. Choosing one **replaces the
  whole effect**: every value and every curve becomes the preset's (a variant has no curves, so the slot's curves
  go; a user preset brings its own curves, re-hung on the event's entry point). One undo step returns the former
  effect with its curves. The slot keeps its place in the chain, its on/off switch and its masks (where it acts),
  and its identity, so an audio effect keeps its tail.
- **The menu reads the truth**: it names the preset the effect currently equals (values and curves), or
  "(custom)" after any edit; picking the shown preset again resets to it.
- **A chain preset joins the chain on the graph**: inserted after the selected node (at the end when none is
  selected), its nodes arrive as one SELECTION, highlighted; the person deletes the extra ones with Delete or moves
  them by dragging, and on the graph v2 (change `fx-graph-dag`) wires them where wanted. Nothing of the existing
  chain is removed by inserting a chain preset.
- **Selecting several nodes with the mouse**: Ctrl+click adds or removes a node, a drag on the empty canvas draws a
  selection frame; Delete removes every selected node in one undo step; dragging one of them moves them all.
- The same head and the same selection work in the event's, the track's and the morpher's graph (one widget).

## Capabilities

### Modified Capabilities

- `ui-reach`: the preset menu on a slot, its reading, one undo step; a chain preset as a selected group; selecting
  several nodes, deleting and moving them.
- `automation`: an effect's preset replaces its curves; a chain preset's curves come with its nodes.

## Impact

- kt-app: the slot form head (`fx_editor.rs` / `widgets`), the variant list shared with the picker
  (`fx_registry.rs`), the comparison "effect equals preset"; `ChainEdit.selected` grows from one `ChainSel` to a
  set; the selection frame, Delete and group drag in `node_editor.rs`; the chain preset's insertion point;
  i18n ×5 ("Preset", "(custom)").
- kt-model: `ops::insert_chain_block_at(position)` beside `append_chain_block` (the same re-hanging of keys).
- No engine or format change.
- Idle: nothing runs; the comparison is computed when the form is drawn.

## Questions to the owner

Decided (2026-11-20, A4, his Q52): an effect's preset replaces the whole effect, curves too, Undo returns them; a
chain preset joins the existing chain on the graph, the extra nodes selected and deleted with the mouse. No open
question.
