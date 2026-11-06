---
id: F-TL-014
title: Frame step and playing only while a key is held
status: done
priority: A
cost: S
category: timeline/navigation
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [6]
ui_rules: [1, 2, 14, 17]
round: 72
point: change r72-timeline-transport-defects
tests: [t1620, t1621, t1622, t1623, t1624]
depends_on: []
trackers: []
services: []
checked: 2026-11-05
---

# F-TL-014 — Frame step and playing only while a key is held

## Essence

Two ways of looking closely at a spot without changing what the transport does afterwards. The arrow keys move one
frame and stop. A held key plays only while it is down, so letting go leaves the picture where it was looked at. Our
space bar plays, boosts on a hold and now also holds to play from pause; Ctrl turns the same gesture backward.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Step Forward/Back | arrows move the cursor by a frame; a hold repeats; the first press stops playback |
| Adobe Premiere | 26 | Step Forward/Back One Frame | ← / →, Shift for five frames; K+J / K+L plays slowly while both are held |
| DaVinci Resolve | 21 | Previous/Next Frame | ← / →; the JKL shuttle; K+L plays at a slow rate while held |
| Final Cut Pro | 12 | Go to Previous/Next Frame | ← / →; K+J / K+L for the held slow play |
| After Effects | 26 | Previous/Next Frame | Page Up / Page Down, not the arrows: the arrows nudge the selected layer |

## The trick behind it

- A frame step must land on the frame GRID, not add a frame's length: a step from a position between two frames that
  adds one length never reaches the next boundary evenly, and the timecode shows a double step.
- "Play while held" in the JKL editors lives on a key CHORD (K plus J or L), because the transport's own key has to
  keep its toggle. A threshold on one key is what YouTube taught people: a short press still toggles.

## Pros

- The arrows are the same in four of the five editors, so nothing has to be learned.
- A threshold keeps the old meaning of a short press and of two quick presses.

## Cons

- A hold from pause now ends in a pause, so a boost cannot be kept by letting go; during playback it stays.

## What users say

NO QUOTES: the research was done from the vendors' official documentation and help of the five editors, from the model's memory (knowledge cutoff 2026-01); a verbatim user quote with a link could not be obtained in this round.

## How it fits Kinotrain

CAPABILITY: openspec/specs/transport/

Done: the arrows and Ctrl+click on Back/Forward step on the fps grid; Space held from pause plays while held, Ctrl reverses it.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app |
| Project format | does not touch |
| Tests that will have to be written | the press machine at each threshold; the step from each place a person acts |
| Depends on | none |
| Breaks compatibility | no |

## Verdict

Take the arrows as in the four NLEs, and the threshold the owner chose (R.71, question 7). The Ctrl+click on
Back/Forward is his "Ctrl instead of Shift". The JKL shuttle stays F-TL-003.

## Sources

- https://helpx.adobe.com/premiere-pro/using/keyboard-shortcuts.html (access date 2026-11-05, from memory)
- https://documents.blackmagicdesign.com/ (access date 2026-11-05, from memory)
