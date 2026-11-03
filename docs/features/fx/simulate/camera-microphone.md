---
id: F-FX-014
title: The camera's microphone — the sound chain of the device the ISP simulates
status: in-progress
priority: C
cost: M
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugin-api, kt-app]
invariants: [1, 2, 11]
ui_rules: [17]
round: kkt-26.11.03b
point: D1
tests: []
depends_on: [F-FX-010, F-FX-012, F-FX-013]
trackers: []
services: []
checked: 2026-11-03
---

# F-FX-014 — The camera's microphone — the sound chain of the device the ISP simulates

## Essence

Camera ISP films the frame again through a simulated lens, sensor and ISP; its looks are devices:
a phone at night, a cheap webcam, a security camera. Each of those devices recorded the sound too,
through its own microphone chain. The paired audio half films the event's sound again through that
chain — the acoustic port, the capsule, the gain control, the noise suppression and the codec — so
the phone look also sounds like the phone that shot it.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | — |
| Premiere Pro | 26 | no paired effect | the audio track can take EQ presets; nothing tied to a video look |
| DaVinci Resolve | 21 | no | Fairlight FX have EQ and dynamics, no device model |
| Final Cut Pro | 12 | "Telephone" voice effect | an EQ-and-distortion preset, not tied to a video effect |
| After Effects | 2026 | no | — |

**Outside the core.** Device emulation is an audio-post specialty.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Audio Ease Speakerphone | 3 | Mic / Speaker / Codec | impulse responses of real microphones and speakers, plus a codec module with a sample rate and a "connection quality" — linear IRs with no gain control or noise suppression in the device |
| McDSP FutzBox | 2 | Synthetic Impulse Response + Lo-Fi | a filter response plus distortion, noise and a bit/rate reducer — an imitation |

## The trick behind it

1. **The device's processing is what you hear, not the capsule.** A phone's MEMS capsule is flat to
   10 kHz; what makes "phone sound" is the automatic gain control lifting the room tone between
   words (breathing), the noise suppressor's spectral gating (musical noise and swallowed word
   tails), and the codec. An IR-based emulation leaves all three out because none of them is linear.
2. **The acoustic port is a Helmholtz resonator.** The hole in the case, the gasket channel and the
   front volume make a resonance f = c/2π·√(A/(V·L)) — a few kHz for a phone — whose peak is the
   "presence" of a phone recording.
3. **Wind is turbulence at the port, not hiss.** Its noise grows with the cube of the wind speed
   and lives below a few hundred hertz; it is what overloads the capsule's acoustic overload point
   and makes the AGC duck.

## Pros

- The three looks get three devices: a phone (stereo MEMS, AGC, NS, AAC), a USB webcam (one electret
  capsule at 16 kHz, AGC, no NS), a security camera (one cheap capsule, 8 kHz G.711 µ-law).
- Every block is bypassable like the ISP's blocks, so a person hears what the AGC or the noise
  suppressor does.

## Cons

- The scene level must be stated (the SPL of the source's full scale): a mic chain is nonlinear, and
  a digital file carries no absolute level.
- The noise suppressor works on a 512-point STFT: a cost per audio second.

## What users say

> NO QUOTES: the Audiofanzine user review of Speakerphone (rated 10/10) is in French and paraphrased
> by the search summary; no verbatim English quote about device-processing (AGC, noise
> suppression) in emulations could be opened (searched "Speakerphone codec review", "phone mic
> emulation AGC plugin", 2026-11-03). The demand is the owner's (`docs/ktprompts/kkt-26.11.03b.txt`).

## How it fits Kinotrain

CAPABILITY: openspec/specs/audio-fx/

### What already exists

The ISP's passport and looks in `src/plugins/fx-video-camera/src/lib.rs` (the `VARIANTS` phone at
night, cheap webcam, security IR), the ISP panel `src/apps/kt-app/src/isp_panel.rs` with stage
blocks and bypasses, the pairing mechanism of F-FX-012, the codec of F-FX-013.

### What is missing

The microphone rows in the camera's passport (one device, one panel), the looks setting them, and
the audio state that runs the chain.

### What hinders

Nothing in the camera's picture moves in time the way a tape or a stream does; the pair shares the
DEVICE (the looks, the sensor seed for the capsule's noise), not a defect schedule.

### Which invariants it touches

1, 2, 11.

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where the main work is | `src/plugins/fx-video-camera` (the microphone chain), kt-app `isp_panel.rs` (the sound blocks) |
| Project format | does not touch it |
| Tests that will have to be written | the port resonance peak; AGC lifts a quiet passage; NS lowers stationary noise; 8 kHz look has nothing above 4 kHz; determinism; bypass identity |
| Depends on | F-FX-010, F-FX-012, F-FX-013 |
| Breaks compatibility | no |

## Verdict

Take: the camera's microphone chain as the audio half of `camera_isp`, set by the same looks.
Cinema raw, wrong CFA phase and no-IR-cut are not devices with a microphone of their own — their
looks leave the microphone at the defaults (a clean camera microphone).

## Sources

1. Audio Ease Speakerphone 3 manual — https://audioease.com/speakerphone/files/speakerphone-3-manual.pdf (the chain: mic, speaker, codec; accessed 2026-11-03)
2. Sound On Sound, Audio Ease Speakerphone review — https://www.soundonsound.com/reviews/audio-ease-speakerphone (codec module: sample rate, connection quality)
3. ITU-T G.711 (µ-law companding, 8 kHz)
4. The WebRTC audio processing module (AGC, noise suppression) — https://webrtc.googlesource.com/src/+/refs/heads/main/modules/audio_processing/
