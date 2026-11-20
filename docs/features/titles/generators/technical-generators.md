---
id: F-TXT-007
title: Technical generators — a sync flash with its beep, safe-area guides
status: in-progress
priority: B
cost: S
category: titles/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugins-builtin, kt-app]
invariants: [8, 11]
ui_rules: [4, 9, 13]
round: features-slow
point: change gen-technical
tests: [t3700, t3701, t3702]
depends_on: [F-TXT-003]
trackers: []
services: []
checked: 2026-11-20
---

# F-TXT-007 — Technical generators — a sync flash with its beep, safe-area guides

## Essence

The owner (`v26.10` line 16, G5, his answer of 2026-10-30 to question 6): "very few generators" —
all four groups, the technical one among them: "sync beep/flash, safe-area grids, timer/counter,
audio tone/noise". The tone and the coloured audio noise exist (generators-placement-and-audio).
This note takes the flash and the guides; the timer/counter becomes a source of the Text generator
(his G3 decision: the timecode is text, controlled as text) — change `text-counter-source`.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Media Generators › Test Pattern; Preview › Overlays › Safe Areas | test patterns and a tone; the safe areas are a preview overlay switch, not a generator — they never reach the render |
| Premiere | 26 | New Item › Bars and Tone, Universal Counting Leader; Program monitor › Safe Margins | the leader has a beep at 2; safe margins are a monitor overlay |
| Resolve | 21 | Generators; Viewer › Safe Area | safe areas are a viewer overlay with presets (action/title %, 4:3 centre cut, custom) |
| Final Cut Pro | 12 | Generators; View › Show Title/Action Safe Zones | an overlay of the viewer |
| After Effects | 26 | Title/Action Safe guides; no flash generator | guides of the composition panel |

## The trick behind it

- The sync flash's picture and beep are ONE event: the frame and the sound must change on the same
  tick, so the beep's gate is computed from the same period as the flash, in material ticks.
- Every editor keeps safe areas as a VIEWER overlay, never in the picture. As a transparent generator
  on a top track it is visible in the preview, removable by muting the track, and exportable when a
  delivery asks for a burnt-in check copy — the one thing an overlay cannot do.
- Guides in fractions of the frame, not pixels: the preview quality must not move them.

## Pros

- A clapper-like A/V sync check on the timeline in one gesture.
- Safe-area guides over the real picture, at any preview size, mutable with their track.

## Cons

- The guides are a layer: a person who forgets to mute it exports them (the overlay route would
  never). A preview-only overlay is a later choice for him.

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own list (G5, quoted above) is the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" in `design.md`.
Two kinds of `src/plugins/generators-test/src/lib.rs` (`syncflash`: live, its beep through the host's `tone_1k` with a gate;
`safearea`: transparent, integer outlines), ids in `BUILTIN_IDS`, frozen in t1048.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | `src/plugins/generators-test`, catalogs |
| Project format | does not touch (plugin rows) |
| Tests that will have to be written | the flash and its beep on one tick; the guides at their fractions, transparent elsewhere; the kinds reachable and captioned |
| Depends on | F-TXT-003 |
| Breaks compatibility | no |

## Verdict

Take the flash and the guides as generators; the counter as a Text source (G3).

## Sources

- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (Media Generators, Safe Areas, access date 2026-11-20)
- https://helpx.adobe.com/premiere/desktop/ (Bars and Tone, Universal Counting Leader, Safe Margins, access date 2026-11-20)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Generators, Safe Area, access date 2026-11-20)
