---
id: F-UI-010
title: Changing an added effect's preset in place
status: candidate
priority: A
cost: S
category: ui/editing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app]
invariants: [3]
ui_rules: [1, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-11-20
---

# F-UI-010 — Changing an added effect's preset in place

## Essence

An effect is added with a preset ("Blur — Heavy"); the editor changes their mind and wants "Soft". Today that
means deleting the slot and adding it again, looking for the preset in the picker. In the editors people use,
the preset is a menu at the top of the effect's own controls: pick another, the values change, the slot stays
where it is with its place in the chain, its mask and its other settings.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | "Preset" drop-down at the top of every plug-in's window | choosing a preset replaces the plug-in's parameters; after an edit the field shows "(Untitled)"; save/delete buttons beside it [NOT VERIFIED live] |
| Adobe Premiere | 26 | Effect presets (Effects panel); Lumetri's own looks | presets are dragged onto a clip as new effects; an existing effect has no preset menu (except Lumetri's look list) [NOT VERIFIED live] |
| DaVinci Resolve | 21 | ResolveFX "Presets" in the inspector header (some effects) | per-effect preset menu where the effect offers one [NOT VERIFIED live] |
| Final Cut Pro | 12 | the effect's "Preset" pop-up in the inspector (Save Effects Preset) | a pop-up beside the effect's name applies a saved preset to that effect [NOT VERIFIED live] |
| After Effects | 26 | Animation Presets (applied to a layer) | applying adds effects; changing an effect's values to another preset means re-applying [NOT VERIFIED live] |

## The trick behind it

**1. The menu must show which preset the values ARE, not which was picked last.** Vegas shows the preset's
name only while the values equal it and "(Untitled)" after any edit — so the field is a truthful reading, and
picking the same preset again is the "reset to that preset" gesture. A field that remembers the last pick lies
after the first tweak.

**2. Changing a preset is an edit of values, not a new slot.** Everything else about the slot — its position in
the chain, bypass, mask, the curves of the rows the preset does not touch — survives, and effect state survives
too (the slot is the same object; invariant 3 here: the audio processor keeps its tail).

## Pros

- One click instead of delete + search + add; the chain and masks stay.

## Cons

- Rows with automation: a preset gives values, a curve gives values over time — one must win. The owner decided
  (2026-11-20, `kkt-26.11.20` A4): an effect's preset replaces the whole effect, curves too; Undo returns them.

## What users say

The owner, `docs/ktprompts/kkt-26.11.07.txt` line 12: "невозможно после добавления эффекта - ПОМЕНЯТЬ ЕГО
ПРЕСЕТ НЕ ПЕРЕСОЗДАВАЯ ЕГО НУЖНО ПОСТОЯННО ЕГО УДАЛЯТЬ ЕСЛИ ПЕРЕДУМАЛ И ЗАДАВАТЬ ЗАНОВО ИЩЯ УЖЕ ПРЕСЕТ В
ВЫПАДАЮЩЕМ МЕНЮ."

NO QUOTES: no web access in this spec session (2026-11-20) for outside quotes; editor rows [NOT VERIFIED].

## How it fits Kinotrain

### What already exists

- The picker's variants per kind (`UiDecl.variants`, `UiVariant { name_key, sets }`,
  `src/libs/kt-plugin-api/src/lib.rs`; `src/apps/kt-app/src/fx_registry.rs`): a variant = the template plus
  its named values.
- The chain preset library (`FxPresetStore`, `src/apps/kt-app/src/user_store.rs`, capability `automation`).
- `SetFx` keeps a slot's identity when its values change (invariant 3).

### What is missing

- A preset menu on an added slot; a reading of which preset the values equal.

### What hinders

- Variants name only the rows they set; "the values equal the preset" must compare against template + sets.

### Which invariants it touches

- **3** — the slot is edited, not re-created. UI rules 1 (reached first, above the form), 17 (one list for the
  picker and the slot), 20 (nothing changes by itself).
- CAPABILITY: openspec/specs/ui-reach/

## Estimate

| What | Answer |
|---|---|
| Scope | S |
| Where | kt-app (the form head), the variants list shared with the picker |
| Project format | none |
| Tests | the slot keeps identity/mask/position; the reading follows edits; curves rule |
| Depends on | nothing |
| Breaks compatibility | no |

## Verdict

Take it: a "Preset" menu at the head of every slot's form listing the kind's variants and the user's
single-slot presets, replacing the whole effect (values and curves) in one undo step and reading "(custom)" after
any edit, as Vegas does. A CHAIN preset is not a replacement: it joins the existing chain on the graph, its nodes
arrive selected, and the extra ones are deleted or moved with the mouse (the owner, A4).

Change `fx-preset-change`, implementation-ready 2026-11-24 on graph v2d: the insertion and its checks are the graph's own
(`node_editor::perform`); the change adds the form's Preset menu and a selection of several nodes.

## Sources

- `docs/ktprompts/kkt-26.11.07.txt` line 12; registry `kkt-26.11.07-index.md` E6.
