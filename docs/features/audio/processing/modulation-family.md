---
id: F-AUD-010
title: Modulation — chorus, flanger, phaser, tremolo, autopan, vibrato
status: done
priority: B
cost: M
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-engine, kt-app]
invariants: [2, 3, 11]
ui_rules: []
round: "R.47"
point: 328
tests: [322, 325]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-AUD-010 — Modulation — chorus, flanger, phaser, tremolo, autopan, vibrato

## Essence

A family of effects where "an LFO turns a parameter": chorus/flanger/vibrato turn the delay, phaser — the phase shift, tremolo — the volume, autopan — the panning. For the editor this means "widen the backing vocals", "a phone-like, watery voice", "a pad that pulses in time". One infrastructure — six audibly different effects.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Chorus / Flange/Wah-Wah / Vibrato / Amplitude Modulation | separate DirectX effects; Amplitude Modulation covers tremolo and autopan with one stereo-phase knob |
| Adobe Premiere | 26 | Chorus/Flanger (one effect), no Phaser | Chorus/Flanger are combined in a preset; no tremolo — people draw the volume with keyframes |
| DaVinci Resolve | 21 | Chorus, Flanger, Phaser (Fairlight FX) | the full set; each has LFO rate/depth and mix |
| Final Cut Pro | 12.3 | Chorus, Phaser, Tremolo (the Logic set) | Logic presets; advanced parameters in an expandable panel |
| After Effects | 26 | Flange & Chorus (audio effect) | one layer effect with voice separation/depth/rate |

**Outside the core.** The mechanics of "an LFO as a modulator of anything" are visible only in DAWs.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Chorus-Ensemble / Phaser-Flanger / Auto Pan | Phaser and Flanger are ONE device with a switch: they share the engine (modulated allpass/delay), the difference is in the range and the feedback |

## The trick behind it

The main subtlety of the family is not in the formulas but in TIME: the phase of the LFO must be a function of position, not a counter of "how many samples have been processed". In a render engine with seek, pre-roll and loop wrap-around, an accumulated phase gives a different sound when rendered cold and after a seek — what DAWs call "free-running vs. transport-synced LFO", and all five use the synced variant in their render paths (the export of two runs is byte-identical). The second detail: in Ableton, phaser and flanger are one device, because a cascade of allpass filters and a modulated short delay are mathematical neighbours; the difference between the effects is the range of times and the sign of the feedback. The third: stereo in chorus/autopan is made not with a second LFO, but by shifting the phase of the same LFO by 90–180° in the right channel.

## Pros

- Six effects on one infrastructure (LFO + fractional delay + allpass): the cost of each next one is only a formula.
- Synced phase gives export determinism for free — without "warming up" the LFO with a pre-roll.
- Tremolo/autopan are memoryless: there is no tail, and seek is trivial.

## Cons

- Vibrato is honest only at 100 % wet (dry + modulated delay = chorus, not vibrato) — the mix parameter is deliberately absent from vibrato.
- A phase tied to the clip's position jumps at the loop wrap-around together with the material — tremolo may "click" in amplitude at the seam (the material also restarts, so this is honest).

## What users say

NO QUOTES: the research was done from the vendors' official documentation and help of the five editors, from the model's memory (knowledge cutoff 2026-01); a verbatim forum fragment with a link cannot be reproduced without live access to the thread — the facts in the table can be checked against the vendors' manuals from the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Chorus/flanger/tremolo — one family with an LFO; the LFO phase is pinned to the event's time.

## Estimate

| What | Answer |
|---|---|
| Scope | M — six nodes on three primitives |
| Where the main work is | kt-fx (Lfo, allpass, nodes) |
| Project format | not touched |
| Tests that will have to be written | render in pieces == render in pieces; seek == cold render; a curve of rate accelerates the modulation; vibrato changes the frequency deviation |
| Depends on | F-AUD-009 (DelayLine), the node chain |
| Breaks compatibility | no |

## Verdict

Take all six in R.47 in one wave: Chorus (voices 1–3, stereo 90°), Flanger (feedback ±), Phaser (2–8 stages, sweep 200–2000 Hz), Tremolo, AutoPan, Vibrato (100 % wet). Waveform — sine only (sawtooth/square are not in this pass). The LFO phase is the integral of the frequency curve over the clip's position: export determinism without a warm-up.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/fairlight (accessed 2026-09-14)
- https://helpx.adobe.com/premiere/using/audio-effects-transitions.html (accessed 2026-09-14)
- https://www.ableton.com/en/live-manual/ (accessed 2026-09-14)

**Implemented in R.47** — items 328 (t325).
