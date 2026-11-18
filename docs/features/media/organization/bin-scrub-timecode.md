---
id: F-MED-008
title: The timecode of the moment under the cursor while scrubbing a bin card
status: done
priority: B
cost: S
category: media/organization
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [8, 12]
ui_rules: [9, 13]
round: features-slow
point: change bin-scrub-timecode
tests: [t3100, t3101]
depends_on: []
trackers: []
services: []
checked: 2026-11-18
---

# F-MED-008 — The timecode of the moment under the cursor while scrubbing a bin card

## Essence

Moving the mouse over a card in the media bin pages through the file: the left edge of the
thumbnail is the file's start, the right edge its end. The person sees the picture but not WHERE in
the file it is, so "the take at about 1:40" cannot be found by the bin alone. The scrub writes the
moment of the file under the cursor in the thumbnail itself, in the program's timecode.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | the Project Media window shows thumbnails without a hover scrub; the moment is read in the Trimmer |
| Premiere | 26 | Hover Scrub (Project panel, icon view) | the thumbnail's width is the whole clip, the pointer's x picks the frame; no timecode over the thumbnail — the moment is read only in the Source Monitor after opening the clip |
| Resolve | 21 | Live Media Preview / hover scrub in the Media Pool | the thumbnail scrubs and the Source viewer follows the hover with its own timecode |
| Final Cut Pro | 12 | Skimming + Skimmer Info (Ctrl+Y) | skimming a browser filmstrip shows a floating info box with the clip name, keywords and the SOURCE timecode at the skimmer |
| After Effects | 26 | no | the Project panel shows one frame; scrubbing happens in the Footage panel |

## The trick behind it

- FCP's skimmer info shows the SOURCE timecode of the file, not the project's: in the browser there
  is no project time yet. The moment a person reads is the one they will later look for in the
  file, so it is the file's own time from its start.
- Premiere's hover scrub is position-proportional over the whole clip, so on a long file one pixel
  is seconds: without a number the scrub says "roughly here" and nothing more. The number is what
  turns the scrub into a way to find a moment, and it costs no decode — the frame under the cursor
  is already chosen from the same x.

## Pros

- The moment comes from the same `hover x → file moment` mapping that picks the thumbnail, so the
  number and the picture cannot disagree.
- Nothing is decoded and nothing is stored: the label exists only while the pointer is over the card.

## Cons

- The scrub picks the nearest stored thumbnail (the ladder of steps), so the picture may be up to
  half a step away from the written moment; the number is the cursor's moment, the picture its
  nearest frame.
- A narrow card has little room: the label must shorten rather than cover the thumbnail.

## What users say

NO QUOTES: searched the Adobe community (hover scrub threads: turning it off, extending it to the
Source Monitor) and Larry Jordan's FCP articles on Skimmer Info; found descriptions of the feature,
no verbatim user statement about a timecode during a bin scrub within the search budget.

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The scrub's own hover-x gives the moment; `bin_panel.rs` writes it with `format_timecode` on a dark chip over the thumbnail, shortened on a narrow card, laid out once per moment. The drag-from-moment half is a question to the owner.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `bin_panel.rs` |
| Project format | does not touch |
| Tests that will have to be written | a harness test that hovers a card and reads the painted timecode; a live scenario |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: the moment under the cursor in the file's own time (as FCP's Skimmer Info), drawn over the
bottom of the thumbnail on a dark backing, shortened on a narrow card. The drag-from-moment half
waits for the owner's answer.

## Sources

- https://www.premiumbeat.com/blog/premiere-pro-hover-scrub/ (access date 2026-11-18)
- https://larryjordan.com/articles/fcp-x-skimming-for-timecode/ (access date 2026-11-18)
- https://support.apple.com/en-us/HT209065 (access date 2026-11-18)
