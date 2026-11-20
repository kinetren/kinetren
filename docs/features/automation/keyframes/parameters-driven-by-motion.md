---
id: F-AUT-014
title: Parameters driven by how time moves — an effect that wakes on acceleration and on scrubbing
status: candidate
priority: C
cost: M
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-app]
invariants: [4, 7, 11, 12]
ui_rules: [16, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-008]
checked: 2026-11-20
---

# F-AUT-014 — Parameters driven by how time moves — an effect that wakes on acceleration and on scrubbing

## Essence

The owner's example: falling letters slow down and speed up; a fisheye should come on while the motion accelerates
or brakes and fade out smoothly while everything moves evenly — without hand-copying the speed curve of another
event. The acceleration may come from the event, from a composition or from a morpher layer. And a second, deeper
case: the same stretch t0–t1 should look one way when played normally and another while it is scrubbed (rewound) —
an effect driven by the PLAYHEAD's motion, not by the edit.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26 | expressions: `speed`, `velocity`, `valueAtTime`; Time Remap | an expression reads another property's velocity (its derivative) and drives a parameter; smoothing by `smooth()`; deterministic per frame [NOT VERIFIED live] |
| Resolve | 21 | Fusion modifiers (Expression, Shake), the Follower | a modifier can compute from another value's change; no playhead-speed input [NOT VERIFIED live] |
| Premiere | 26 | none (keyframes only) | — |
| VEGAS Pro | 2026 | none (envelopes only) | — |
| Final Cut Pro | 12 | Motion behaviours (Rate, Link) published | a parameter linked to another's rate of change in Motion [NOT VERIFIED live] |

## The trick behind it

**1. Acceleration is the derivative of a curve the program already has.** The event's speed is a curve; its slope per
second is the acceleration, and a "wake on change" envelope is a peak-hold of |acceleration| with a release. Computed
from the curve in closed form at any tick, it needs no state and is the same at a seek, in a scrub and in an export.

**2. The playhead's motion is not part of the edit.** No editor renders "what the person did with the transport": an
export plays once at 1×. An effect driven by scrubbing is therefore a preview-only input, and it must say so; its
frames must not be kept as if they were the edit's.

## Pros

- No copying of another event's speed keys; the fisheye follows edits of the speed curve by itself.
- The same channels feed the falling-letters physics later (F-FX-005).

## Cons

- A preview-only input makes the preview differ from the export by design — it must be named.

## What users say

The owner, `docs/ktprompts/v26.10.txt` lines 160–164: the fisheye that activates on acceleration and deceleration
and switches off smoothly in even motion, "не хочется руками вылавливать и подстраиваться под уже существующие кривые
тормажения-скольжения которые находятся у другого события"; the acceleration may be on a composition or a morpher;
"в момент обычного проигрывания некоторого участка t0-t1 - у нас fisheye не должен активироваться а вот в момент
перемотки тот же участок … уже будет с fisheye … Нужно очень хорошо над этим подумать и как это связано например с
физикой."

NO QUOTES: no web access in this spec session (2026-11-20); the editor rows are [NOT VERIFIED] for the same reason.

## How it fits Kinotrain

### What already exists

- Links (R.61, `src/libs/kt-model/src/links.rs`): a property follows another by address with an expression; a link
  can already read another event's `rate` curve (the speed of the warp).
- The speed profile and its integrals (`src/libs/kt-model/src/warp.rs`, `ClipWarp`), compiled curves.
- The transport's play rate, the space boost and the scrub in the player (`src/libs/kt-engine/src/player.rs`).

### What is missing

- Derived channels of a speed: acceleration and a "speed change" envelope; the speed of a composition instance and
  of a morpher layer's warp as link sources; a preview-only transport channel and its keying.

### What hinders

- A kept composite (invariant 4) must not be reused across different transport values.

### Which invariants it touches

- **4** — frames that read the transport are keyed by it or not kept.
- **7, 11** — edit-derived channels are pure functions of the model; the export reads the transport channel at 1×.
- **12** — nothing at rest.
- CAPABILITY: openspec/specs/automation/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-model (derived channels, link sources), kt-engine (the transport channel, keys), kt-app (the link picker) |
| Project format | yes, by content (new link sources) |
| Tests | acceleration channel equals the curve's slope; the envelope is seek-independent; the export ignores the transport |
| Depends on | F-AUT-008 (links) |
| Breaks compatibility | no |

## Verdict

Take it as link sources: from any speed (an event's, a composition instance's, a morpher layer's) the channels
"speed", "acceleration" and "speed change" (a peak-hold with attack and release, closed form, stateless); and a
preview-only "transport speed" channel (play rate and scrub) that an export reads as 1×, named as preview-only in the
link picker.

## Sources

- `docs/ktprompts/v26.10.txt` lines 160–164 (TD2); registry `v26.10-index.md` TD1, TD2.
