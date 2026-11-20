---
id: F-AUT-013
title: Copy, default and own presets for every entity, and "take it from that event"
status: candidate
priority: B
cost: M
category: automation/presets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-app]
invariants: [3]
ui_rules: [17, 20, 24]
round: null
point: null
tests: []
depends_on: [F-AUT-004]
checked: 2026-11-20
---

# F-AUT-013 — Copy, default and own presets for every entity, and "take it from that event"

## Essence

Every thing that has values — an effect, a chain, a curve, a transition, a generator or title, a mask, a
camera, a track's chain, a morpher patch — answers the same four questions in the same place: copy it, paste
it, pick one of the built-in presets, pick or save my own. And without saving anything: point at another event
and take its curve, chain or effect from there.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | plug-in presets; Paste Event Attributes / Selectively Paste | presets per plug-in; pasting the copied event's FX, pan/crop and envelopes onto selected events, with a checklist [NOT VERIFIED live] |
| Adobe Premiere | 26 | Paste Attributes; Effect Presets | a dialog with a checklist (motion, opacity, effects, keyframes "scale attribute times") [NOT VERIFIED live] |
| DaVinci Resolve | 21 | Paste Attributes; applying a grade from another clip | a checklist of attributes; in Color a grade is taken from another clip by middle-click [NOT VERIFIED live] |
| Final Cut Pro | 12 | Paste Attributes; Save Effects Preset | a checklist of attributes, keyframes "maintain timing / stretch" [NOT VERIFIED live] |
| After Effects | 26 | Animation Presets; copy/paste keyframes; the pick whip | the pick whip LINKS a property to another (live) [NOT VERIFIED live] |

## The trick behind it

**1. "Take it from that event" is a paste with a CHECKLIST, and the checklist is the feature.** Every editor
that does it well asks WHAT to take — effects, motion, opacity, curves — and how to fit the keys (keep timing
or stretch to the target). Without it, "take from" overwrites what the person did not mean to take.

**2. One contract, or the gaps are random.** A program grows copy/presets entity by entity, and the person meets
an entity without them at the worst moment (our FX buffer exists; transitions and curves have no presets —
registry A3). The cure is one head menu every entity form uses, so a new entity cannot ship without it.

**3. A live reference and a copy are different gestures.** AE's pick whip makes a LINK; Paste Attributes makes
a COPY. Ours has links (R.61) — "by reference without archive" must say which of the two it is.

## Pros

- Every entity copyable and presettable in the same place; "take from" without the clipboard.

## Cons

- A checklist is one more step for the quick case — its default must be "everything of this kind".

## What users say

The owner, `docs/ktprompts/v26.10.txt` lines 71–72: "нужно чтобы пользователь мог сохранять свои
кривых/пресетов fx-chain/конкретных эффектов, а также - возможность без отдельного сохранения в архив - взять
кривую/пресет fx-chain/конкретных эффектов из другого события(ссылка)", "У всех(fx-chain/кривые/переходы и т.д.
и т.п.) - у всех есть возможность копирования кривых, выбор дефолтных пресетов, и пользовательских пресетов."

NO QUOTES: no web access in this spec session (2026-11-20). The search would be the editors' forums and r/premiere, r/davinciresolve, r/VEGASPro for the feature by its name in the table; the editor rows are [NOT VERIFIED] for the same reason, and the next wave verifies them live.

## How it fits Kinotrain

### What already exists

- Chain presets with curves in the user store (`FxPresetStore`, `src/apps/kt-app/src/user_store.rs`, F-AUT-004).
- The FX clipboard `FxSlotBlock`, `FxChainBlock`, `EnvelopeBlock` (`src/libs/kt-model/src/ops.rs`): a paste re-hangs
  keys on the target's entry.
- Links (R.61, `src/libs/kt-model/src/links.rs`) — a live reference to another event's property.
- Title presets, the kind picker's variants (F-AUT-006, F-AUT-012), the slot's Preset menu (change `fx-preset-change`).

### What is missing

- Presets of curves, transitions, masks, cameras, morpher patches; one head menu; "take from event…" with a
  checklist.

### What hinders

- Entities with different time domains (curves in material time, transitions in window time): the paste of keys
  uses each one's domain rule.

### Which invariants it touches

- **3** — pasting into an existing slot keeps it (values, not re-creation). UI rules 17, 20, 24.
- CAPABILITY: openspec/specs/automation/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-model (blocks for the missing entities), kt-app (one head menu, preset store kinds, the pick gesture) |
| Project format | none (presets in the user store; blocks are clipboard) |
| Tests | every entity form has the menu (a guard); take-from with a checklist; key domains on paste |
| Depends on | F-AUT-004 (done) |
| Breaks compatibility | no |

## Verdict

Take it as one contract: a head menu for every entity with values (Copy, Paste, Presets ▸ built-in / mine, Save
as preset…, Take from event…), one preset store with a kind per entity, and "Take from event…" as a pick on the
timeline followed by a checklist of what to take — a copy by default, "Link instead" for curves (R.61).

## Sources

- `docs/ktprompts/v26.10.txt` lines 71–72 (A2, A3); registry `v26.10-index.md`.
