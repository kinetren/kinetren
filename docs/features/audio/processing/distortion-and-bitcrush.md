---
id: F-AUD-011
title: Distortion / saturation and Bitcrusher
status: done
priority: B
cost: S
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-app]
invariants: [1, 11]
ui_rules: []
round: "R.47"
point: 327
tests: [322, 323, 324]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-AUD-011 — Distortion / saturation and Bitcrusher

## Essence

Controllable "dirt": soft saturation (an overdriven voice/guitar, a "hot" bass) and lo-fi degradation (bit-depth +
sample rate reduction — the sound of a walkie-talkie, an old game, a phone). For the editor this is "make the voice
angry", "the sound from a speaker", "the VHS aesthetic for sound" — a paired effect to our video VHS.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Distortion (DirectX FX) | several saturation curves to choose from, drive/output |
| Adobe Premiere | 26 | Distortion (Audio Effects) | one Amount knob + Tone; the kit includes "radio/phone" presets based on EQ+dist |
| DaVinci Resolve | 21 | Distortion (Fairlight FX) | a choice of type (soft/hard clip, tape), drive, a post-filter |
| Final Cut Pro | 12.3 | Distortion / Bitcrusher (a Logic set) | both as separate Logic plugins; Bitcrusher has resolution and downsampling |
| After Effects | 26 | none | there is no built-in audio distortion — people move the sound to Audition |

**Outside the core.** Bitcrusher as a standalone device — from DAW culture.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Redux | two independent blocks: bit depth (quantization) and downsample (sample hold); it shows clearly that these are TWO different degradations |

## The trick behind it

Level compensation: in all mature implementations the drive raises the input, but the output is compensated (by a divider
or by auto-normalization), so the knob changes the CHARACTER, not the loudness — otherwise an A/B comparison makes no sense,
and the user confuses "louder" with "better". Bitcrusher has two independent axes (depth in bits and hold frequency), and
both are audibly different: bits give a noise floor, downsample gives aliased overtones; merging them into one knob is a
typical error of cheap implementations. A third detail: the hold grid of the downsample must be tied to the material, not
to the moment of launch — otherwise two renders of one clip give different artifacts.

## Pros

- Both effects are memoryless (except the sample hold) — zero tail, trivial seek, the cheapest class of audio effects.
- softclip x/(1+|x|) is algebraic: no libm surprises between platforms, determinism for free.

## Cons

- An honest distortion asks for oversampling against aliasing from saturation; without it, mirror frequencies are audible
  on bright sources (accepted as a deliberate price of the first version — as in most NLE implementations).

## What users say

NO QUOTES: the research was done from the official documentation and the help of five editors from the memory of the model (knowledge cutoff 2026-01); a verbatim forum fragment with a link cannot be reproduced without live access to the thread — the facts of the table are verifiable by the vendors' manuals from the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Distortion/bitcrush are pointwise nodes without state, byte-for-byte determinism.

## Estimate

| What | Answer |
|---|---|
| Scope | S |
| Where the main work is | kt-fx (two nodes of ~60 lines each) |
| Project format | not touched |
| Tests that will have to be written | the order of the slots is audible (quiet→overdrive ≠ overdrive→quiet); the crusher raises the noise floor; rate 48000 is transparent |
| Depends on | the node chain |
| Breaks compatibility | no |

## Verdict

Take both in R.47: Distortion (drive_db/tone/mix, softclip with compensation /√g) and Bitcrusher (bits fractional 1–16,
rate_hz, mix; the grid is from the clip's position). Oversampling is outside the scope (a separate pass, if aliasing gets
in the way in practice).

## Sources

- https://helpx.adobe.com/premiere/using/audio-effects-transitions.html (accessed 2026-09-14)
- https://support.apple.com/guide/logicpro/welcome/mac (accessed 2026-09-14)
- https://www.ableton.com/en/live-manual/ (accessed 2026-09-14)

**Implemented in R.47** — docs/porting-notes.md item 327 (t323/t324); the further life of the feature is there.
