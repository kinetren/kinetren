---
id: F-TL-015
title: Cut out the region's interior from the keyboard
status: done
priority: B
cost: S
category: timeline/assembly
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [17, 18]
round: features-slow
point: change region-cut-key
tests: [t3102, t3103]
depends_on: []
trackers: []
services: []
checked: 2026-11-18
---

# F-TL-015 — Cut out the region's interior from the keyboard

## Essence

A region is drawn over several events; the person wants the part inside it gone and the rest left in
place (no ripple). The program can do it — "Cut out the region interior" in the menu of a track's empty
space — but only there: no key, no Edit menu item, so a person who works from the keyboard draws a
region with I/O and then reaches for the mouse and a right click on an empty spot. The owner asked
for the action from the keyboard and named the key: `C`.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Delete with a time selection | Delete removes the selected events' part inside the time selection; with Auto Ripple off the gap stays |
| Premiere | 26 | Lift (`;`) / Extract (`'`) | Lift removes what lies between In and Out on the targeted tracks and leaves the gap; Extract closes it |
| Resolve | 21 | Delete Selected (Backspace) on In/Out | with In and Out marked, Backspace lifts the range on the auto-select tracks, Shift+Backspace / Delete ripples it |
| Final Cut Pro | 12 | Range selection (R) + Delete / Shift+Delete | Delete on a range removes it and ripples the storyline; Shift+Delete replaces it with a gap |
| After Effects | 26 | Lift Work Area / Extract Work Area (Edit menu) | the work area is the range; Lift leaves a gap in every layer, Extract closes it |

## The trick behind it

- Every one of them makes the gap-keeping cut and the closing cut TWO keys next to each other
  (Premiere `;` and `'`, Resolve Backspace and Shift+Backspace, FCP Shift+Delete and Delete). The
  gap-keeping one is the safe default for a non-ripple editor, and it is the one we have: our
  `punch_out_region` keeps the right remainder at its place and its source alignment.
- The range is the editor's own range (In/Out, work area, time selection), never a separate
  selection tool: a person draws it with the same keys they then cut with. Ours is the region, drawn
  with I and O, so the cut belongs next to them on the keyboard.

## Pros

- One key closes the keyboard loop I → O → C; nothing new is drawn, the command already exists and
  goes through `apply_batch_restoring` (crossfade handles come back in the same undo step).

## Cons

- A bare letter is easy to hit by accident; the cut is one undo step, and it is said in the Edit
  menu with its key.
- The other layouts (Premiere, Resolve, Final Cut, After Effects, VEGAS) bind C to tools we do not
  have (razor, blade); the command stays unbound there rather than borrowing their letter for
  another meaning.

## What users say

NO QUOTES: searched Adobe community threads on Lift/Extract (the Extract key going missing from
the default layout in 23.2, Extract not answering with the timeline focused); found reports, no
verbatim statement read within the search budget.

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-controls/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
`Cmd::PunchOut` bound to `C` in our layout only; the key, Edit › Cut out the region interior and the empty-space item run `punch_out_action` (gap-keeping, one undo step); no region is said, not silent.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `controls.rs`, `toolbar.rs`, `timeline.rs`, `app.rs` |
| Project format | does not touch |
| Tests that will have to be written | the key and the Edit item give the same cut; the key list names it |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: a command "Cut out the region interior" with `C` in our layout, in the Edit menu with its chord,
and the chord in the empty-space item; the same body. Unbound in the other layouts. A ripple variant
waits for F-TL-001.

## Sources

- https://helpx.adobe.com/au/premiere-pro/using/default-keyboard-shortcuts.chromeless.html (access date 2026-11-18)
- https://community.adobe.com/t5/premiere-pro-discussions/keyboard-shortcut-for-quot-extract-quot-when-timeline-in-focus/m-p/14607660 (access date 2026-11-18)
