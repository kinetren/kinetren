---
id: F-FX-016
title: A simulated device on a whole video track sounds on the track's linked audio track
status: done
priority: B
cost: S
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-fx, kt-otio, kt-app]
invariants: [1, 2, 3, 11]
ui_rules: [4, 9, 17, 20]
round: kkt-26.11.18b
point: change track-device-sound
tests: [t3230, t3231, t3232, t3233, t3234, t3235, t3236]
depends_on: [F-FX-012, F-FX-013, F-FX-014, F-UI-008]
trackers: []
services: []
checked: 2026-11-20
---

# F-FX-016 — A simulated device on a whole video track sounds on the track's linked audio track

## Essence

The owner (kkt-26.11.18b line 29, asked "VHS Tape, Datamosh, Camera ISP на видеодорожке без звука — как
быть?"): "Дать звук отдельно" — the option "the track's device node gets a sound lane wired to the
audio track you pick in its form". On an event the device's sound half already sits on the linked
audio event (F-FX-012); a whole video track has no linked audio, so the tape put on the track was
silent. The track gets one: its sound track.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Track FX (video), Track FX (audio) | separate chains per track kind; nothing ties a video track to an audio track |
| Premiere Pro | 26 | Source patching V1/A1, track effects | the source patcher pairs V1 with A1 by default when linked media lands; track effects are per kind |
| DaVinci Resolve | 21 | Auto track selector; Fairlight track FX | linked clips land on Vn/An by number; audio track FX live in Fairlight, apart from video |
| Final Cut Pro | 12 | roles, connected clips | no track pairing at all (magnetic timeline) |
| After Effects | 2026 | no | layers carry their own audio; no device effect |

## The trick behind it

1. **Every NLE already pairs Vn with An — at the moment linked media lands.** Premiere's source
   patcher targets V1+A1 together, Resolve's auto track selector puts the picture on V1 and the
   sound on A1, Vegas inserts the audio on the track below. The pairing exists in the person's head
   and in the default placement, but no editor keeps it as a link the effects can use. So the
   useful default for "which audio track is this video track's sound" is the audio track of the
   same number — where the file's sound landed.
2. **The link must be one-to-one.** Two video tracks mirroring devices into one audio chain would
   fight over its n-th slot; an audio track is the sound of at most one video track.

## Pros

- The track's device is heard as the deck plays it back, its dropouts at the moments the picture
  drops (the same rows, the same timeline time).
- The sound half has its own switch, as on an event.

## Cons

- A video track and an audio track are one more relation to keep valid (removal, reels).

## What users say

NO QUOTES: no other editor has a device simulation on a track, so there is nothing to quote about
its sound; searched (2026-11-20) "track effect video audio linked track premiere" — only source
patching threads. The demand is the owner's (kkt-26.11.18b line 29).

## How it fits Kinotrain

CAPABILITY: openspec/specs/audio-fx/
Done for VHS Tape and Camera ISP (`Track.sound`, the event's pair rule, the track window's pair graph); Datamosh on a track waits for a track stream stage (found-defects 820).

## Estimate

| What | Answer |
|---|---|
| Scope | S |
| Where the main work is | kt-model (the link, the pair rule), kt-app (the track window's pair editor) |
| Project format | a track's sound link raises the file by content |
| Tests that will have to be written | the link and the mirror both ways; the half's own switch; the track stage sounds the deck; the file and OTIO; the window |
| Depends on | F-FX-012, F-FX-013, F-FX-014 |
| Breaks compatibility | no |

## Verdict

**Done** (change `track-device-sound`, archived 2026-11-20; his answer "Дать звук отдельно", 2026-11-18): a video track links one audio track as its
sound (by default the one of the same number), and its devices' sound halves are real slots of that
track's chain, mirrored by the event rule.

## Sources

1. Adobe, Premiere Pro — "Source patching and track targeting" (help page, V1/A1 targets)
2. Blackmagic, DaVinci Resolve 21 manual — Edit page, auto track selector
3. The owner's answer, `docs/ktprompts/kkt-26.11.18b.txt` line 29
