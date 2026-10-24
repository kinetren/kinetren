---
id: F-AUD-013
title: Pitch shift — pitch without changing duration
status: done
priority: B
cost: L
category: audio/processing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-engine, kt-otio, kt-app]
invariants: [1, 2, 3, 5, 11]
ui_rules: []
round: "R.48"
point: 339
tests: [322, 337, 338, 339, 340]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-AUD-013 — Pitch shift — pitch without changing duration

## Essence

Raise or lower the pitch of an event's sound without touching its length: the voice becomes lower or higher, the music moves to another key, and the event stays exactly where it was and of the same duration. The reverse of the existing Slowed+Reverb, where pitch moves together with speed. For the editor this means "make the voice deeper", "fit the backing track to the key" and "remove the childlike timbre from a sped-up take".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Pitch Shift / Elastique | An event effect and an event property; several algorithms (élastique Pro/Efficient/Soloist), the mode is chosen per event |
| Premiere Pro | 26 | Pitch Shifter (effect) | A track/clip effect in Essential Sound; separately Audition Stretch with a choice of Realtime/Rendered |
| DaVinci Resolve | 21 | Pitch (Fairlight) | A slider in semitones/cents on a Fairlight track; the algorithm is hidden from the user |
| Final Cut Pro | 12 | Pitch (audio inspector) | A clip parameter in semitones/cents, tied to the Speed → Preserve Pitch setting |
| After Effects | 26 | Pitch & Time (none of its own) | It has no granular pitch of its own; time-stretch changes the pitch, and precise work is done in Audition |

**Outside the core.** Without Ableton one cannot explain why TWO algorithms are needed: it is the only one that shows the choice of mode to the user as a regular part of the work, rather than as a "quality setting".

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12 | Warp Modes | Beats/Tones/Texture/Complex — different algorithms for different material, switched on the clip; Complex Pro is a vocoder, Beats is granular with transient preservation |
| RubberBand / élastique | — | libraries | The industry standard: time-domain for rhythmic material, a phase vocoder with phase-locking for harmonic material |

## The trick behind it

**The algorithm is not about "quality" but about a CHOICE FOR THE MATERIAL, and it is audible on transients.** A granular (WSOLA) method cuts the stream into grains and stitches them together by maximum correlation: a drum stays a hit, but dense polyphony gives a light "rustle" at the joins. A phase vocoder computes the spectrum and shifts the frequencies of the bins: pads and synths move cleanly, but a hit smears into a "puff" — the classic vocoder artifact, which advertising calls "softness". The editors hide this choice in different ways: Vegas shows three élastique modes on the event, Resolve shows nothing, Ableton makes the choice an ordinary part of working with a clip.

**Second: pitch costs more than it seems, because of the analysis window.** Both algorithms look FORWARD (grain/FFT window), that is, they introduce a delay into the signal path. A host that cannot compensate it either shifts the track relative to the others or is forced to render the effect offline — hence Audition's split into Realtime and Rendered.

**Third: pitch and speed are different axes, but they live in one node.** In Vegas "Lock to stretch" ties them back together, because the editor sometimes wants to speed up without the "chipmunk" effect, and at other times wants to raise the pitch without speeding up.

## Pros

- Pitch becomes an independent axis: slowing an event no longer dictates the timbre, and the "slowed" aesthetic stops being the only available sound.
- Two algorithms on one node cover both extremes of material (speech/hit against pad/polyphony) without inventing a third DSP.
- Pitch working AFTER the warp on a clip-local stream does not touch the warp's monopoly on consuming the source: the event's duration does not move, so prefetch, boundaries and curves stay as they were.

## Cons

- The analysis window = lookahead: without a mechanism of end-to-end delay compensation the effect either shifts the event or has to read the source ahead (the latter is more honest, but widens the prefetch window).
- No algorithm is free of artifacts: WSOLA rustles on dense material, the vocoder smears hits. This is a property, not a setting.
- Formants "move" together with the pitch: a voice raised by five semitones sounds cartoonish. Formant correction is separate work.
- Choosing the engine with a single switch per project is simpler to implement, but less flexible than a mode per event (as in Vegas and Ableton).

## What users say

NO QUOTES: the research was done from the official documentation and help of the five editors, from the model's memory (knowledge cutoff 2026-01); a verbatim forum fragment with a link cannot be reproduced without live access to the thread — the facts in the table can be checked against the vendors' manuals from the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/audio-fx/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Pitch shift — two engines to choose from in the settings; neutral byte-for-byte, a render in pieces == in pieces.

## Estimate

| What | Answer |
|---|---|
| Scope | L — a round (lookahead + two engines) |
| Where the main work is | kt-fx (the node and both DSPs), kt-fx/audio/mod.rs (lookahead) |
| Project format | does not touch the version: a new settings field with a default is not written into old files |
| Tests that will have to be written | neutral byte-for-byte; a render in pieces == in pieces; the spectral peak at the expected frequency; switching the engine changes the sound and survives saving |
| Depends on | the lookahead mechanism (the same wave) |
| Breaks compatibility | no |

## Verdict

Take both engines at once: they cover different material, and the choice between them is part of the editor's work, not a hidden "quality setting" (the direct decision of the owner). The switch is at the level of the PROJECT: one edit sounds with one algorithm, and two variants can be compared with one click, without going through events. The neutral value must be the same sound byte-for-byte as before — otherwise an effect added "to try it" silently changes the export.

## Sources

- https://www.vegascreativesoftware.com/us/vegas-pro/ (accessed 2026-09-14)
- https://helpx.adobe.com/audition/using/time-pitch-manipulation.html (accessed 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve/fairlight (accessed 2026-09-14)
- https://support.apple.com/guide/final-cut-pro/ (accessed 2026-09-14)
- https://www.ableton.com/en/live-manual/ (accessed 2026-09-14)

**Implemented in R.48** — items 337, 339–340
(t337–t340). Formant correction and a mode per event (instead of a project setting) were not done.
