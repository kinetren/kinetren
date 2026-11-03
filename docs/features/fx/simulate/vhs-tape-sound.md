---
id: F-FX-012
title: The sound of the VHS tape — the linear track and Hi-Fi FM from the same deck
status: done
priority: B
cost: M
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-plugin-api, kt-model, kt-fx, kt-engine, kt-app]
invariants: [1, 2, 3, 11]
ui_rules: [17]
round: kkt-26.11.03b
point: D1
tests: [t1460, t1461, t1462, t1463, t1464, t1465, t1466, t1467, t1468, t1485]
depends_on: [F-FX-009]
trackers: []
services: []
checked: 2026-11-03
---

# F-FX-012 — The sound of the VHS tape — the linear track and Hi-Fi FM from the same deck

## Essence

The owner (2026-11-03, `docs/ktprompts/kkt-26.11.03b.txt`): the VHS Tape simulation "must have a
paired audio filter". A tape that recorded a picture recorded its sound on the same oxide, through
the same deck: the sound of an event that wears VHS Tape is the sound that deck plays back. A head
clog that drops the picture in a field must be heard in that field; the capstan that bends the time
base must wobble the pitch. So the sound is not a "lo-fi preset" but the second half of the same
simulated device, edited in the same deck panel.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | VHS looks are video FX only (Boris FX / third-party); the sound is untouched |
| Premiere Pro | 26 | no | the video "VHS" templates and third-party looks have no audio half |
| DaVinci Resolve | 21 | no | Fairlight has EQ, distortion and modulation, no tape model; the VHS look is a Fusion/OFX node |
| Final Cut Pro | 12 | no | audio "Lo-Fi" style effects exist, unrelated to any video effect |
| After Effects | 2026 | no | no audio processing beyond basic effects |

**Outside the core.** The mechanism lives in audio plugins that model tape machines; none of them is
tied to a picture.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| CHOW Tape Model | 2.11 | Loss / Hysteresis / Flutter | a physical reel-to-reel model: Jiles–Atherton hysteresis for bias and saturation, loss filters from gap, spacing, thickness and speed (1–50 ips), flutter and wow |
| Wavesfactory Cassette | 1.x | Cassette | a cassette deck model (hiss, wow and flutter, saturation, dropouts) — a look, no physical units |
| Sapphire S_VHSDamage | 2025 | VHS Damage | video only |

## The trick behind it

1. **The two VHS audio systems obey different physics, and a deck picks one.** The linear track is
   recorded by a STATIONARY head along the tape edge at the tape's own speed (PAL SP 23.39 mm/s, LP
   half, EP a third): its wavelength at 10 kHz is 2.3 µm, so gap and spacing losses cut the top
   hard and LP sounds dull — the same law that blurs the luma, but at a speed two hundred times
   lower. Its wow and flutter is the CAPSTAN's (the tape moves under a still head). Hi-Fi audio is
   FM (1.3 / 1.7 MHz carriers, ±150 kHz) recorded by heads ON THE DRUM, depth-multiplexed under
   the video: its flutter is the drum's (tiny), its noise is FM noise that breaks into clicks below
   the FM threshold, and at every head switch the carrier passes from one track to the next —
   a misaligned joint gives the field-rate "buzz" (50/60 Hz), worse with tracking error and worn
   heads.
2. **A dropout is a length on the tape, not a time.** An oxide defect 29 µm long (6 µs at the
   video writing speed of 4.85 m/s) lasts 1.2 ms on the linear track at 23.39 mm/s — the same
   defect is a speck in the picture and a click in the linear sound.
3. **Tracking hurts the Hi-Fi sound more than the picture** (the US patent on tracking control:
   at the optimum for video, the audio is inadequately reproduced): the narrow deep layer loses its
   carrier first.

## Pros

- A defect is heard where it is seen: the clogged head of the field that drops the picture loses
  the Hi-Fi carrier in the same field, so the sound breaks into FM noise at the field rate.
- The panel stays one deck: speed, heads, tracking and the tape's condition are set once, and the
  sound follows.
- The linear track's dullness at LP comes out of the wavelength, not a low-pass knob.

## Cons

- The Hi-Fi path is simulated as complex FM baseband at 8× the audio rate: a real cost per audio
  second (the carrier itself at 1.3 MHz is not sampled).
- The picture's dropouts are per video line; the sound reads the same defect field, so a person who
  sets 500 dropouts/s hears a crackle — physically right, surprising.

## What users say

> The loss params are quite powerful tone shapers.
> — nilhartman, KVR Audio forum, CHOW Tape Model thread, 2020-05-23, https://www.kvraudio.com/forum/viewtopic.php?p=7790175

## How it fits Kinotrain

CAPABILITY: openspec/specs/audio-fx/
Done as `vhs_tape_sound` (`src/plugins/fx-video-tape/src/sound.rs`, change `sim-audio-pairs`): both VHS audio systems, driven by the deck rows and the picture's own line table at the timeline time; the pair is mirrored across the link group.

## Estimate

| What | Answer |
|---|---|
| Scope | M — the pairing mechanism plus the VHS sound |
| Where the main work is | `src/plugins/fx-video-tape` (the audio state), kt-model (the mirroring), kt-fx/kt-engine (the origin) |
| Project format | does not touch it: a new plugin id and rows written only off their default |
| Tests that will have to be written | a clogged head's field is lost in picture and sound at the same time; LP top loss; flutter period; bypass identity; determinism |
| Depends on | F-FX-009 |
| Breaks compatibility | no |

## Verdict

**Done** (round kkt-26.11.03b, D1, 2026-11-03). Take (change `sim-audio-pairs`): the audio half of `vhs_tape` as a simulation of both VHS audio
systems, driven by the same deck rows and the same timeline-keyed time base and dropout field.

## Sources

1. Sencore Tech Tip 190, "How VCR Hi-Fi Stereo Works" — https://xdevs.com/doc/Sencore/Sencore_Tech_Tips/TT190p%20-%20How%20VCR%20Hi-Fi%20Stereo%20Works.pdf (1.3/1.7 MHz carriers; accessed 2026-11-03)
2. Audio magazine, November 1984, VHS Hi-Fi — https://gammaelectronics.xyz/audio_11-1984_vhs.html (depth multiplex, ~4 µm deep layer)
3. "Magnetic tape recording: depth-multiplex audio" — https://television-and-video-engineering.blogspot.com/2015/09/magnetic-tape-recordingdepth-multiplex.html (±150 kHz deviation, tracking critical in LP)
4. US 5243474 — https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/5243474 (video and audio tracking optima differ)
5. US 8639092 — https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8639092 (unsuppressed head switching gives a 50/60 Hz buzz)
6. CHOW Tape Model — https://github.com/jatinchowdhury18/AnalogTapeModel (loss filters from gap, spacing, thickness, speed)
