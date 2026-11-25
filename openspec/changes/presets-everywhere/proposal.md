# Proposal: presets-everywhere

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` A2, A3 (partial). Note F-AUT-013. Tests reserved:
> t3904–t3907.

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

(Q68, Q69 of `docs/owner-checks.md`; re-read against `fc60a5d9`, 2026-11-25.)

**Your items whole** (`docs/ktprompts/v26.10.txt` lines 71–72): "нужно чтобы пользователь мог сохранять свои кривых/пресетов fx-chain/конкретных эффектов, а также - возможность без отдельного сохранения в архив -
взять кривую/пресет fx-chain/конкретных эффектов из другого события(ссылка)," and "У всех(fx-chain/кривые/переходы и т.д. и т.п.) - у всех есть возможность копирования кривых, выбор дефолтных пресетов, и
пользовательских пресетов."

**Now**: chains have own presets and a clipboard (Ctrl+C/V on the graph), five built-in chains; single effects have variants in the picker (their menu: `fx-preset-change`, designed); a curve is copied and pasted
into another effect's curve; one parameter can follow another live (R.61: "Link" in the curve). Transitions, masks, cameras, layers and titles have neither presets nor copying of values. Your answer of
2026-10-30 (question 4) was about the FILE: an identical chain is written once, in the program every copy stays its own.

**Q68 (Q1). "Взять … из другого события(ссылка)": a copy or something that goes on following?** Example: event A has Blur + Colour with a curve; you point at A from 40 other events.
- (a) **A copy; for a single curve the dialog also offers "Follow"** (*recommended*): the 40 get their own Blur + Colour. Later you change A's blur: only A changes; to spread it you select the 40 and point at A again (one
  gesture). Deleting A changes nothing. In the file it is still one chain (`project-scale`).
- (b) Always follow, whole chains too: changing A's blur changes 41 events at once. Then: an event of the 40 cannot have its own blur value until it is unlinked; deleting or cutting A needs a rule (the first
  follower becomes the source); a follower inside a composition cannot follow an event outside it (as links today); copying a follower to another project brings a copy.
- (c) Ask each time: Copy or Follow, for chains as well: (a) plus a shared chain as an object of its own ("chain style") with a name, edited in one place. The widest, and the largest to build.

**Q69 (Q2). "Выбор дефолтных пресетов": for which kinds first?** Built in today: five chains, the variants of every effect and transition kind.
- (a) **Curves (ease in/out, bounce, pulse, ramps; about eight) and the transitions' variants as presets; masks, cameras and layers start with your own only** (*recommended*).
- (b) Built-in sets for every kind at once (also about six masks, eight camera moves, a few layers): nothing of what you ask waits, and the kinds whose looks must be invented make it about twice as long.
