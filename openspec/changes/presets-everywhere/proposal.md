# Proposal: presets-everywhere

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` A2, A3 (partial). Note F-AUT-013. Test
> numbers: pending a new block from the orchestrator.

## Why

The owner, `v26.10` lines 71–72: "нужно чтобы пользователь мог сохранять свои кривых/пресетов
fx-chain/конкретных эффектов, а также - возможность без отдельного сохранения в архив - взять кривую/пресет
fx-chain/конкретных эффектов из другого события(ссылка)"; "У всех(fx-chain/кривые/переходы и т.д. и т.п.) - у
всех есть возможность копирования кривых, выбор дефолтных пресетов, и пользовательских пресетов." Today chains
(with curves) have user presets and a clipboard; single effects get a Preset menu by `fx-preset-change`; curves,
transitions, masks, cameras, generator values and morpher patches have neither presets nor one copy gesture.

## What Changes

- **One head menu for every entity with values** — an effect slot, a chain (picture, sound, a track's), a
  curve, a transition, a generator/title, a mask list, a camera, a morpher patch: Copy · Paste · Presets ▸
  (built-in, then mine) · Save as preset… · Take from event…. A guard holds that every entity form draws it.
- **One preset store with a kind per entity** (the user store; factory presets in the binary, parsed by the
  same code): chain and slot presets exist; curve, transition, mask, camera, generator and patch presets are
  added. A curve preset stores its keys relative (domain 0..1 of its span, values in the row's unit) and is
  fitted to the target row's range.
- **Take from event…** (Vegas/Premiere "Paste Attributes", by pointing instead of the clipboard): the cursor
  becomes a picker, a click on another event (or transition, camera block, track header) opens a checklist of
  what it has — picture chain, sound chain, their curves, geometry, masks, level/gain, generator values — with
  "keep timing / stretch to length" for keys; OK applies to the current target (or every selected event) in one
  undo step. For a curve the dialog also offers "Link instead" (a live link, R.61) — Q1.

## Capabilities

### Modified Capabilities

- `automation`: presets for every entity kind, curve presets in relative time, take-from as a checklisted paste
  with the key domain rules.
- `ui-reach`: the one head menu on every entity form and its guard; the pick gesture.

## Impact

- kt-model: clipboard blocks for transitions, masks, cameras, generator values, patches (`ops.rs`, the
  `FxSlotBlock`/`EnvelopeBlock` family); `take_from(source, target, Checklist)` producing commands.
- kt-app: `PresetStore` (generalising `FxPresetStore` with a kind), the head menu widget, the pick mode, the
  checklist dialog (a registry window row), i18n ×5.
- Format: none (presets in the user store; blocks travel on the clipboard as today).
- Idle: nothing runs; the pick mode draws only while the pointer moves.

## Questions to the owner

**Q1. "ссылка" — a copy or a live link?** Your words: "взять кривую/пресет … из другого события(ссылка)". Your
answer C2c (2026-10-30) chose references only in the FILE, every copy independent in the program. Links (R.61)
already make one property follow another live.
- (a) **Take-from copies by default; for a curve the dialog offers "Link instead" (it then follows the source
  live)** (*recommended*).
- (b) Take-from always links (live), for every entity — the source's later edits change every taker.

**Q2. Built-in presets for the new kinds** — which first?
- (a) **Curves (ease in/out, bounce, pulse, ramp up/down) and transitions (each kind's variants as presets);
  masks, cameras and patches start with "mine" only** (*recommended*).
- (b) Built-ins for every kind at once.
