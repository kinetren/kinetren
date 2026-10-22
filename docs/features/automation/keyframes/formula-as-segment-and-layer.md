---
id: F-AUT-010
title: Formula on a segment and a formula layer over keys
status: done
priority: A
cost: M
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: yes
crates: [kt-model, kt-otio, kt-app]
invariants: [11]
ui_rules: [5, 9, 16, 20]
round: R.67
point: "change r67-curves-stills-import (chronicle frozen at §470)"
tests: [t784, t787, t787b, t787c, t787d, t787e, t792, t792b]
depends_on: [F-AUT-007, F-AUT-008, F-AUT-009]
trackers: []
services: []
checked: 2026-09-26
---

# F-AUT-010 — Formula on a segment and a formula layer over keys

## Essence

The editor sets keys with the mouse or in a table, gives the stretch between two keys a shape (straight, third order, step, zigzag) — or a FORMULA for that stretch; over the whole curve writes one more formula, which sees the value of the keys `k` and adds its own; all of this is visible on one graph and is removed separately: remove the layer — the keys with shapes remain; remove the stretch's formula — the zigzag remains. No step erases the previous one.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Fade type / Set Type on an envelope key | the shape is a property of the stretch between keys, a closed list (Linear/Fast/Slow/Smooth/Sharp/Hold); there are no formulas at all |
| Adobe Premiere | 26 | Temporal interpolation + Ease handles | shape on the key, Bezier handles; no expressions — only in After Effects |
| DaVinci Resolve | 21 | Keyframe interpolation (Curves) / Fusion Expressions | on the Edit page — shape on the key; in Fusion an expression REPLACES the input's animation entirely, the keys are disabled but not deleted (Modifier → Expression) |
| Final Cut Pro | 12.3 | Interpolation (Linear/Smooth) | only the shape on the key; no formulas |
| After Effects | 26 | Expressions over keyframes | the expression receives `value` — the values of the keys beneath it — and returns its own; the keys remain, and the expression switch returns them; there is no separate "formula on a stretch" — only `linear()/ease()` by time inside an expression |

**Outside the core.** Blender shows what happens when a formula layer is a separate object: Drivers
and F-Curve Modifiers (Generator, Noise, Cycles) stack ON TOP of the keys.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 | F-Curve Modifiers (Generator/Noise/Cycles) + Drivers | the modifiers are applied to the result of the keys in list order, each has a frame range (Restrict Frame Range) and an influence 0..1; a driver is a Python expression that replaces the value entirely |

## The trick behind it

1. **In After Effects an expression SEES the keys as `value` and does not delete them.** This is the only model where "changed my mind — back to the keys" costs one click: the expression can be switched off, the keys are always beneath it. Our previous code put the formula as the SOURCE and erased the keys — the Resolve Fusion model (a Modifier replaces the input), which no editor outside Fusion expects.
2. **Blender is the only one whose "layer" has a RANGE OF FRAMES.** That is how it expresses "apply something to separate areas" (Restrict Frame Range on a modifier). We get the same capability more cheaply: a formula on a STRETCH between keys already has a range — the keys define it, and inserting a key splits the area in half without a second mechanism.
3. **None of the five core editors has a formula that is the SHAPE OF A STRETCH.** The closest is `ease()/linear()` inside an AE expression, where the boundaries of the stretch have to be written as numbers. For us the boundaries of the stretch are the keys, and the formula receives `s` (the fraction of the path), `a` and `b` (the ends): "jitter on this piece" — the owner's words about the zigzag (R.61) — extends to any formula.

## Pros

- Nothing is erased: keys, shapes, layer and link are independent layers with a fixed order (keys → shapes → layer), each removed by its own action (rules 5, 20).
- "The combination of all this" (the owner, R.67) is expressed without a new window and without a stack with an order: two places for a formula — the stretch and the layer — cover "area" and "everything".
- The format changes by content (v13 only when there is a stretch-formula or a layer): files without them stay byte-stable; the `Formula` source of old files is read as a layer over empty keys — the semantics are the same.

## Cons

- The two places for a formula must be explained in the interface: "stretch formula" lives in the stretch menu next to the zigzag, "layer" — in the curve block; without help in the block (F-HLP-034) the person will again ask "how do I assign them".
- A formula on a stretch is entitled to produce a break at a key (like a step) — this is its freedom, but also its trap; the graph must show this.
- A stack of modifiers with time ranges (Blender) remains unexpressed; the owner chose "stretch + layer" consciously.

## What users say

NO QUOTES: the reconnaissance was conducted from the official documentation and help of the five core editors and Blender (Expressions/`value` in AE, Modifier → Expression in Fusion, F-Curve Modifiers in Blender); a search of forums and trackers was not conducted in this round — the source of the requirement here is not foreign users, but the owner's direct words (`docs/user-feedback.md`, R.67: "I can EITHER draw the points myself… or a formula, or apply something to separate areas AND ALSO A COMBINATION OF ALL OF THIS"; "I may have changed my mind or made a mistake and would like to go back to the zigzag?").

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A formula is a variant of the stretch shape (`s`, `a`, `b`, `t` from the left key) and an optional curve layer (`k`, `t`, `src`) over the keys; the order is fixed, they are removed separately; `t` is from the start of the material everywhere; resolution into keys before the audio stream runs in the same pass as the links.

## Estimate

| What | Answer |
|---|---|
| Size | M — the model (shape and layer, the resolver, validation of overrides), format v13, a block in the widget |
| Where the main work is | kt-model (`SegShape::Formula`, `ParamCurve.layer`, `links.rs`), kt-otio (the envelope), kt-app (`automation.rs`, `widgets::curve_block`) |
| Project format | affected by content: v13 for a stretch-formula or a layer; without them — the previous bytes |
| Tests that will have to be written | a stretch formula against an oracle and determinism; a layer over keys and its removal; `t` from the material in rendering and in the preview; an instance override with a formula; a format round-trip and the rejection by an old reader |
| Depends on | F-AUT-007 (the evaluator), F-AUT-008 (the link — `src`), F-AUT-009 (stretch shapes) |
| Breaks compatibility | not forward; backward yes — a v13 file is rejected by an old build by its version |

## Verdict

**Accepted by the owner's decision (R.67).** The After Effects model ("the expression sees `value`,
the keys beneath it are alive") plus a formula as the shape of a stretch — and not Blender's stack of modifiers:
the owner was offered both, and the first was chosen as "they combine, are removed separately".
Rejected: a formula-source that erases the keys (our previous code; the Fusion Modifier model) —
because "changed my mind" would cost all the manual work; a stack with areas — because
the area is already defined for us by the keys of the stretch.

## Sources

- Adobe After Effects Help: "Expression basics" — `value`, enabling/disabling an expression
  with the keys preserved (checked 2026-09-26 against the 26.x help).
- Blackmagic DaVinci Resolve 21 Reference Manual, Fusion: "Modifiers → Expression".
- Blender 5.2 Manual: "F-Curve Modifiers" (Generator, Noise, Restrict Frame Range), "Drivers".
- VEGAS Pro 2026 Help: "Envelope keyframe types".
- `docs/user-feedback.md`, R.61 and R.67 — the owner's words and decisions.
