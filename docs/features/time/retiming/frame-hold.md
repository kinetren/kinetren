---
id: F-SPD-006
title: Frame hold — stop an event on a frame and go on from it
status: in-progress
priority: B
cost: S
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-app]
invariants: []
ui_rules: [17, 20, 21]
round: 72
point: null
tests: [t1627]
depends_on: [F-SPD-002]
trackers: []
services: []
checked: 2026-11-05
---

# F-SPD-006 — Frame hold — stop an event on a frame and go on from it

## Essence

The editor stands on a frame and says "hold here". The picture stops for a while, then goes on from the same
frame. The event keeps being one piece of story: nobody typed a speed, made two cuts or counted seconds. What
differs between editors is what happens to everything after the hold.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Velocity envelope at 0 %; Insert Freeze Frame at Cursor | a key at zero on the event's envelope, or a still made from the frame |
| Adobe Premiere | 26 | Add Frame Hold / Insert Frame Hold Segment / Frame Hold Options | the insert splits at the playhead and adds a held piece of a default length, pushing the rest |
| DaVinci Resolve | 21 | Retime Controls → Freeze Frame | a hold segment inside the same clip, its length changes by dragging its edge |
| Final Cut Pro | 12 | Hold (Shift+H), Add Freeze Frame (Option+F) | Hold is a retime segment of the clip; the freeze is a separate still connected to the story |
| After Effects | 26 | Freeze Frame (time remap), Hold keys | a layer's Time Remap held by a hold keyframe |

## The trick behind it

- Premiere keeps TWO different commands under similar names: "Add Frame Hold" ends the event there, "Insert Frame
  Hold Segment" keeps going. People pick the wrong one because the names differ by one word.
- In Resolve and FCP the hold is a part of the retime of ONE clip, so it stretches by its edge and the sound of the
  piece goes silent with it. In Premiere it is a separate piece, so a trim on it is an ordinary trim.
- The default length differs: Premiere's follows the still-image default in Preferences, so people get a surprise
  of five seconds; the industry gives no fixed number.

## Pros

- One step of undo instead of two cuts and a typed speed.
- As an existing rate 0 it adds no format change and no new render path.

## Cons

- Without a ripple the material past the end of the event is no longer reached; the person trims it back.

## What users say

NO QUOTES: the research was done from the vendors' official documentation and help of the five editors, from the model's memory (knowledge cutoff 2026-01); a verbatim user quote with a link could not be obtained in this round.

## How it fits Kinotrain

CAPABILITY: openspec/specs/transport/

### What already exists

`rate = 0` of a time-warp slot is a freeze since R.57 (`src/libs/kt-model/src/warp.rs`), `clip_slice` in
`src/libs/kt-model/src/project.rs` is the one rule of pieces.

### What is missing

The command: a split, a second split and a hand-typed speed were needed.

### What hinders

The piece after the hold must read material from the frozen frame, and on an event with a speed curve its curves
must travel with that anchor.

### Which invariants it touches

None. Checklist rules 17 (one body), 20 (no neighbour moves), 21 (the industry name).

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-model (the pieces), kt-app (the command) |
| Project format | does not touch |
| Tests that will have to be written | the pieces on a plain, a curved and a reversed event; undo; the menu |
| Depends on | F-SPD-002 |
| Breaks compatibility | no |

## Verdict

Take, as the Premiere "Insert Frame Hold Segment" meaning without its ripple: the owner's rule 20 says nothing moves
by itself, so the event keeps its length and the end of the material is cut.

## Sources

- https://helpx.adobe.com/premiere-pro/using/create-freeze-frame.html (access date 2026-11-05, from memory)
- https://support.apple.com/guide/final-cut-pro (access date 2026-11-05, from memory)
