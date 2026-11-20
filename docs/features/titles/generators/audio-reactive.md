---
id: F-TXT-010
title: Audio-reactive generators — waveform, spectrum, level rings
status: in-progress
priority: B
cost: L
category: titles/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-plugin-api, kt-model, kt-media, kt-engine, kt-plugins-builtin, kt-app, kt-cli]
invariants: [2, 4, 5, 7, 11]
ui_rules: [4, 9, 13, 16, 17]
round: features-slow
point: change gen-audio-reactive
tests: [t3714, t3715, t3716, t3717, t3718, t3719]
depends_on: [F-TXT-003]
trackers: []
services: []
checked: 2026-11-20
---

# F-TXT-010 — Audio-reactive generators — waveform, spectrum, level rings

## Essence

The owner (`v26.10` G5, his answer of 2026-10-30, question 6) named four generator groups. The last
one is audio-reactive: a waveform, a spectrum and level rings that move with the sound of the edit. A
generator so far drew from its own values and the time only. It could not hear anything.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no audio-reactive generator; third-party plug-ins (NewBlue, BorisFX) | not built in |
| Premiere | 26 | no built-in visualizer; "Audio Spectrum" lives in After Effects, through Dynamic Link or a Motion Graphics template | done in AE |
| Resolve | 21 | Fusion: Audio Waveform tool (the waveform of a chosen audio clip), expressions on an audio file's level | the tool names an audio FILE and reads it itself, not the timeline mix |
| Final Cut Pro | 12 | no built-in visualizer; third-party (Motion templates) | not built in |
| After Effects | 26 | Effect › Generate › Audio Spectrum, Audio Waveform; "Convert Audio to Keyframes" | both effects name an audio LAYER and read that layer's own sound (its source, with its time remapping), not the comp's mix after effects; Convert Audio to Keyframes bakes the level into a slider |

## The trick behind it

- Every editor that has it reads one audio SOURCE, never the final mix after effects. AE's Audio
  Spectrum names a layer. Resolve's Audio Waveform names a clip. The analysis is then a pure function
  of a file and a time, so it can be computed once, ahead, and kept. A mix after effects would tie
  the picture to the audio render thread.
- AE's spectrum has a frequency range, a band count, a maximum height and "hold" (peaks fall slowly).
  The falling peaks make it read as music rather than flicker. Here the hold is a fold over the series'
  last half second, so a frame stays a pure function of its time (seek, determinism).
- The waveform at a frame is the samples AROUND that moment. A level is RMS, not peak, or every
  click jumps.

## Pros

- The series is computed once per file in the background, then kept beside the file's other analyses
  (peaks, loudness) and keyed by the file's fingerprint. A frame then costs a lookup.
- A generator listens to a TRACK (by number), not to one event. A music bed cut into pieces on A1
  drives one visualizer across the cuts, and a moved or replaced piece is heard at once.
- Preview and render read the same series. A render computes a missing one before it starts, so its
  bytes never depend on whether the background job had finished.

## Cons

- **It reacts to the raw file, not to the mix after effects.** A reverb, a fade or a level line on the
  event does not change what the visualizer sees. This is a question to the owner (below).
- A reversed or speed-changed audio event is read at its plain material time: the visualizer does not
  follow the warp (written in the design; it needs the warp map on the series' time).

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own list (G5) is the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" in `design.md`.
The series (`src/libs/kt-plugin-api/src/audio_series.rs`) is a plain data type: per audio burst (1920 ticks) the RMS of
each channel, the peak, 16 log bands and 32 waveform points, one byte each. `src/libs/kt-media/src/listen.rs` computes it from a
short-lived source of its own (invariant 5's offline door) and stores it behind one small store trait.
The store is today's sidecar and moves with the caches into the project folder later (A14). kt-engine's
`gen_sources` turns a listening generator's track into a FEED: pieces of series in the generator's
material time. The feed is part of `GenState`, so a new series, a moved piece or another track retunes
the source like any value edit, and the frame key moves (invariant 4). The kinds draw on the compute
device from one storage buffer of the frame's audio values.

## Questions to the owner

- **The raw file or the mix?** (`docs/owner-checks.md`) The visualizer hears the FILE of the events on
  the track it listens to, as AE and Resolve do. The other way, option (b), reads what the track
  actually sounds like after its effects and level. It needs the audio render thread's final mix
  handed to the picture, and a render must render the sound first. It is possible later.

## Estimate

| What | Answer |
|---|---|
| Scope | L — analysis, store seam, the feed through the engine, three kinds, the background job |
| Where the main work is | kt-media (analysis), kt-engine (`gen_sources` feed), a plugin family |
| Project format | does not touch (plugin rows) |
| Tests that will have to be written | the series is deterministic and round-trips the store; the feed follows a moved event; a frame changes with the sound and only with it; render computes a missing series; the kinds are created and captioned |
| Depends on | F-TXT-003 |
| Breaks compatibility | no |

## Verdict

Take: option (a). The series is computed per file and the generator listens to a track; three kinds.
Option (b), the mix after effects, waits for the owner.

## Sources

- https://helpx.adobe.com/after-effects/using/generate-effects.html (Audio Spectrum, Audio Waveform, access date 2026-11-20)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Fusion Audio Waveform, access date 2026-11-20)
