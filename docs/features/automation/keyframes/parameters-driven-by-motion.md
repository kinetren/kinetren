---
id: F-AUT-014
title: Parameters driven by the edit's speed — an effect that wakes when its container accelerates or brakes
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

# F-AUT-014 — Parameters driven by the edit's speed — an effect that wakes when its container accelerates or brakes

## Essence

The owner's example: falling letters slow down and speed up; a fisheye should come on while the motion accelerates
or brakes and fade out smoothly while everything moves evenly — without hand-copying the speed curve of another
event. The acceleration may come from the event, from a composition or from a morpher layer. And the deeper case: the
same stretch t0–t1 of a composition looks one way in an instance played at normal speed and another in an instance
whose speed curve accelerates — the owner confirmed (2026-11-22, B7, "Скорость монтажа") that "перемотка" means the
edit's speed, not the playhead's motion.

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

**2. The speed that matters is the container's.** An effect inside a composition is played by the instance's time
map; two instances of one composition at different speeds must give the same content two looks. The value is the
outer map's rate (the product of nested maps), known per instance and pure, so preview and render agree.

## Pros

- No copying of another event's speed keys; the fisheye follows edits of the speed curve by itself.
- The same channels feed the falling-letters physics later (F-FX-005).

## Cons

- A reel whose content reads its outer speed can no longer share one sub-plan between instances of different speeds.

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
- A reel's sub-plan keyed by instance (`src/libs/kt-engine/src/video_plan.rs`, `NestKey`).

### What is missing

- The outer map's channels (speed, acceleration, speed change) resolved per instance and through nested maps.

### What hinders

- A link may not cross a reel's boundary (R.61): the outer speed is a channel of the time map, not a link outside.

### Which invariants it touches

- **4** — an instance's plan is keyed by its outer profile when its content reads it.
- **7, 11** — the channels are pure functions of the edit; preview and export agree.
- **12** — nothing at rest.
- CAPABILITY: openspec/specs/automation/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-model (the outer map, channels), kt-engine (per-instance keys), kt-app (the link menu) |
| Project format | yes, by content (new link sources) |
| Tests | the channels equal the closed form; seek equals continuous; two instances give two looks |
| Depends on | F-AUT-008 (links) |
| Breaks compatibility | no |

## Verdict

Take it as the outer map's channels: an effect inside a composition, under a morpher layer or on an event reads the
speed at which its container plays it (composed through nesting), its acceleration and a smooth "speed change"
envelope — per instance, pure, the same in preview and render.

## Sources

- `docs/ktprompts/v26.10.txt` lines 160–164 (TD2); registry `v26.10-index.md` TD1, TD2.
