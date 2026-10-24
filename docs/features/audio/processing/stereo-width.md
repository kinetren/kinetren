---
id: F-AUD-012
title: Stereo width (mid/side width)
status: done
priority: B
cost: S
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-app]
invariants: [1]
ui_rules: []
round: "R.47"
point: 327
tests: [322, 324]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-AUD-012 — Stereo width (mid/side width)

## Essence

One knob "narrower ↔ wider": 0 — mono, 1 — as recorded, 2 — spread apart. For the editor this means "narrow the music to fit the voice", "make the backing track mono so the dialogue sits in the centre", "inflate the ambience". The classic mid/side matrix.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Pan/Expand | combines panning and width; expand — the same s-channel |
| Adobe Premiere | 26 | Stereo Expander | one width knob + the centre frequency of the "inflation" |
| DaVinci Resolve | 21 | Stereo Width (Fairlight FX) | width 0..2, a separate m/s balance |
| Final Cut Pro | 12.3 | Stereo Spread (Logic) | a spectral variant: the bands are spread between the channels |
| After Effects | 26 | Stereo Mixer | essentially the volume/pan of the channels; there is no honest m/s width |

## The trick behind it

The real subtlety is an identity at unity: a naive implementation (l=m+s·w; r=m−s·w through a float matrix) at width=1 is NOT byte-equal to the input (two floating-point operations per sample), and mature implementations either short-circuit unity or honestly document "always processes". The second detail: Premiere "inflates" not everything but only from a configurable frequency upward — the bottom stays mono so the bass does not spread (mono compatibility). The third: width is applied BEFORE the event's pan, otherwise the pan moves an already inflated picture and the centre drifts away.

## Pros

- One parameter, zero state, zero tail — an extremely cheap effect with great practical demand ("music narrower, voice in the centre").
- The matrix is already written and verified in the project (the width of the reverb tails).

## Cons

- width > 1 worsens mono compatibility (s grows against m) — the sum in mono is quieter and "phases"; this is a property of the technique, not a defect of the implementation.

## What users say

NO QUOTES: the research was done from the official documentation and help of the five editors, from the model's memory (knowledge cutoff 2026-01); a verbatim forum fragment with a link cannot be reproduced without live access to the thread — the facts in the table can be checked against the vendors' manuals from the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Stereo width — an M/S node without a split; one Gain = an exact multiplier.

## Estimate

| What | Answer |
|---|---|
| Scope | S |
| Where the main work is | kt-fx (the node, ~30 lines) |
| Project format | not touched |
| Tests that will have to be written | width 0 ⇒ l==r; width 1 ⇒ byte-pass-through; width 2 grows the s-energy |
| Depends on | the node chain |
| Breaks compatibility | no |

## Verdict

Take it in R.47: one width knob 0..2 with an explicit pass-through at 1.0. The frequency split of Premiere ("bottom mono") — we do not take: one knob covers the editing tasks; the split is material for a separate note, if asked for.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/fairlight (accessed 2026-09-14)
- https://helpx.adobe.com/premiere/using/audio-effects-transitions.html (accessed 2026-09-14)

**Implemented in R.47** — item 327 (t324).
