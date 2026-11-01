---
id: F-AUD-014
title: Sound generators — tone, coloured noise, sweep, beep train, silence
status: done
priority: B
cost: M
category: audio/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, audition-26, logic-pro-12]
we_have: yes
crates: [kt-plugin-api, kt-media, kt-app]
invariants: [1, 11]
ui_rules: [17, 18]
round: kkt-26.11.01
point: 4
tests: [t560, t1155, t1156, t1157, t1158, t1159, t1160, t1162, t1163, t1164, t1165, t1166, t1167]
depends_on: [F-TXT-003]
trackers: []
services: []
checked: 2026-11-01
---

# F-AUD-014 — Sound generators — tone, coloured noise, sweep, beep train, silence

## Essence

An audio event with NO source file: it computes its sound. A sine at a set frequency and level
(the line-up tone at the head of a master), white/pink/brown noise (monitor calibration, a
stand-in for room tone, a masking bed), a sine sweep (checking a filter, a speaker, a resampler),
a beep train (the sync pip, the countdown beep without the countdown picture) and silence (a
placeholder that holds a place on an audio track). On the track it is an ordinary audio event:
trimmed, cut, faded, given FX. Until this change the only sounds a person could make were the
1 kHz tone hidden inside SMPTE bars and the beep inside the countdown leader.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no (video Generated Media only) | a tone or noise is made in Sound Forge (Tools → Synthesis) and imported `[NOT VERIFIED]` |
| Premiere | 26 | Bars and Tone | a project item: SMPTE/HD bars with a 1 kHz tone at −12 dBFS by default; the level is changed with the clip's Audio Gain, the frequency is fixed |
| Resolve | 21 | Fairlight oscillator (test tone) | a patchable oscillator, 20 Hz–10 kHz, −50…+10 dB, presets 100/440/1 k/2 kHz, white and pink noise (Resolve 15 feature guide); it feeds a bus, it is not an event on a track |
| Final Cut Pro | 12 | no | generators are pictures; tone is made in Logic's Test Oscillator and imported |
| After Effects | 26 | Tone (effect, Audio category) | an effect on a layer: a waveform (sine, triangle, saw, square) and up to five frequencies |

**Outside the core.** The full tool lives in audio editors, and the verdict needs to see what a
sound generator looks like when it is taken seriously.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Audition | 26 | Generate → Tones / Noise / DTMF, Insert Silence | Tones: base frequency, five partials, Sweep Frequencies from a Start to an End tab; Noise: white/pink/brown, Mono/Inverse/Spatial Stereo/Independent Channels, intensity 2–40 |
| Logic Pro | 12.3.1 | Test Oscillator | a plug-in: sine, square, noise, pink noise, sine sweep with a time and start/end frequencies |

## The trick behind it

- **The level of a tone and the level of a noise are two different numbers.** dBFS is defined by
  the sine (AES17: a full-scale sine is 0 dBFS); a noise has no peak, and a −20 dBFS noise must
  read −20 on the same RMS meter as a −20 dBFS tone. Premiere's users ran into the reverse for
  years: the tone reads −12 in Premiere and hotter on a hardware meter.
- **A sweep's phase is the integral of its frequency**, not `sin(2π·f(t)·t)`: the naive form
  sweeps twice as far and clicks at every repeat. A log sweep (equal time per octave) is the one
  measurements use; a linear one spends 90 % of its time above 2 kHz.
- **Seek must not change the bytes.** Pink and brown noise are filters with state, so the sound at
  a point depends on everything before it — a generator that keeps that state gives other samples
  after a seek than in a straight play. The noise must be a pure function of the sample number
  (a counter-based hash, and the Voss–McCartney rows taken as sample-and-hold values of that
  hash), or a render and a preview disagree.

## Pros

- A generator on a track is an event, so the line-up tone, a placeholder and a sync pip are cut,
  moved and faded like any audio — Resolve's oscillator, patched to a bus, is none of that.
- One sine kernel for every kind keeps the phase rule in one text: a beep train is the tone with a
  gate, and the countdown leader's beep is the same pip.

## Cons

- Without partials and waveforms (Audition, After Effects) a tone is a sine only; a square or saw
  is a distortion FX on top.
- A noise from held hash rows is pink/brown in octave bands, not a textbook 1/f line: the rows
  step, and each step is a little white-ish energy above its rate.

## What users say

> In Premiere you simply click bars and tone and they appear. Does anyone know an EASY way to
> generate tone in Resolve?
> — Blackmagic forum, "Bars and Tone", https://forum.blackmagicdesign.com/viewtopic.php?p=677498

## How it fits Kinotrain

CAPABILITY: openspec/specs/generators/
Done in `generators-placement-and-audio`: five sound kinds as the `generators-audio` plugins (a burst from the global sample number, one sine with SMPTE, AES17 levels), created from Edit, a track's right-click menu at the clicked place, or the bin.

## Estimate

| What | Answer |
|---|---|
| Scope | M — one change: a contract hook, a family crate, the menu |
| Where the main work is | a new family crate under `src/plugins/`, `kt-media` generator source |
| Project format | does not touch: a new kind is a new plugin id |
| Tests that will have to be written | byte determinism and seek, level in dBFS, spectral slope, the menu |
| Depends on | F-TXT-003 |
| Breaks compatibility | no |

## Verdict

Taken and done (generators-placement-and-audio, 2026-11-01): tone, white/pink/brown noise,
log/linear sweep, beep train and silence as generator kinds on an audio track, each a plugin;
partials, waveforms and DTMF stay out until asked for.

## Sources

- https://helpx.adobe.com/audition/desktop/effects-reference/generate-tones-effect.html (access date 2026-11-01)
- https://forum.blackmagicdesign.com/viewtopic.php?p=677498 (access date 2026-11-01)
- https://community.adobe.com/t5/premiere-pro-discussions/generated-audio-tone-6db-adobe-help-says-12db-client-asking-for-20dbfs-help-please/m-p/11064957 (access date 2026-11-01)
