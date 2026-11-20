# Proposal: fx-preset-change

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word (registry `kkt-26.11.07-index.md`
> E6, "on his word"). Tests reserved: t3595–t3596.

## Why

The owner, `kkt-26.11.07` line 12: "невозможно после добавления эффекта - ПОМЕНЯТЬ ЕГО ПРЕСЕТ НЕ ПЕРЕСОЗДАВАЯ
ЕГО НУЖНО ПОСТОЯННО ЕГО УДАЛЯТЬ ЕСЛИ ПЕРЕДУМАЛ И ЗАДАВАТЬ ЗАНОВО ИЩЯ УЖЕ ПРЕСЕТ В ВЫПАДАЮЩЕМ МЕНЮ." Today the
kind's variants (`UiDecl.variants`) are offered only by the picker when a slot is ADDED. Note F-UI-010.

## What Changes

- **A "Preset" menu at the head of every effect slot's form** (above the rows — UI rule: presets first),
  listing the same variants the picker offers for this kind, then the user's single-slot presets of this kind
  from the preset library. Choosing one sets the slot's values to what the picker would have given (the
  kind's defaults + template + the variant's values) in ONE undo step: the slot keeps its position, bypass,
  masks and identity (its audio state is not reset).
- **The menu reads the truth**: it shows the preset whose values the slot currently equals, or "(custom)"
  after any edit; picking the shown preset again resets to it.
- **Rows with curves** (Q1): a row that carries a curve keeps its curve; the menu says how many.
- The same head works in the node editor's slot view and the morpher patch (one widget, UI rule 17).

## Capabilities

### Modified Capabilities

- `ui-reach`: the preset menu on a slot, its reading, one undo step.
- `automation`: applying a preset to a slot keeps curves on curved rows (Q1 a).

## Impact

- kt-app: the slot form head (`fx_editor.rs` / `widgets`), the variant list shared with the picker
  (`fx_registry.rs`), the comparison "values equal preset", i18n ×5 ("Preset", "(custom)").
- No model, engine or format change (the command is `SetFx` of the slot's values).
- Idle: nothing runs; the comparison is computed when the form is drawn.

## Questions to the owner

**Q1. A row that has a curve, when another preset is chosen?** Your words: "ПОМЕНЯТЬ ЕГО ПРЕСЕТ НЕ
ПЕРЕСОЗДАВАЯ ЕГО". A preset gives values; a curve gives a value over time.
- (a) **The curve stays; the preset sets the other rows; the menu says "2 rows keep their curves"**
  (*recommended*: animation work is not lost by a menu pick; rule 20).
- (b) The preset replaces everything, curves removed (Vegas-like; the animation is lost, Undo brings it back).
