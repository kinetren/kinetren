---
id: F-AUT-012
title: Previews in the generator and transition kind pickers
status: done
priority: B
cost: S
category: automation/presets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [5, 7, 8, 12]
ui_rules: [1, 7, 12, 17]
round: features-slow
point: change kind-picker-previews
tests: [t3108, t3109, t3110]
depends_on: [F-AUT-006]
trackers: []
services: []
checked: 2026-11-18
---

# F-AUT-012 — Previews in the generator and transition kind pickers

## Essence

The effects picker shows every candidate as a cell on the person's own frame (F-AUT-006). The two
other pickers of a KIND — the generator's kind in the event window and the transition's kind in its
window — were bare lists of names: "Iris", "Clock", "Barn door", "Leader", "Checker". The owner:
"нет превью генераторов при выборе в окне, у переходов так же нету … в половине программы нормальный
интерфейс (как у эффектов …) а у половины обрубки" (`v26.10` line 17, row G6).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Transitions window; Media Generators window | each transition and generator is a column of preset thumbnails; a transition's thumbnail animates an A→B sample on hover; a generator's is a still of the preset — never the person's footage |
| Premiere | 26 | Effects panel › Video Transitions; Essential Graphics | transitions are names in folders with no picture; the result is seen after applying |
| Resolve | 21 | Effects Library › Transitions / Generators, Hover Scrub Preview | the cell is a static icon; with Hover Scrub Preview on, hovering a transition plays it in the VIEWER on the two clips at the nearest cut; a generator plays in the viewer as itself |
| Final Cut Pro | 12 | Transitions browser, Generators browser | thumbnails; skimming a thumbnail animates the transition in the cell over sample images, not the person's clips |
| After Effects | 26 | — | no transitions as a kind; generators are effects in the Effects & Presets list without pictures |

## The trick behind it

- A transition has no single picture: its look is a PROGRESS. The editors that show it either animate
  a sample (Vegas, FCP) or play it in the viewer over the real cut (Resolve). A still of a transition is
  useful only at a progress where both sides show — the middle of the window, not its edges, where
  every kind is the same frame A or B.
- Resolve is the only one that shows the transition on the PERSON's two clips, and it pays with a
  pipeline in the viewer behind a switch. Our cells already render "the frame after the click" on the
  offline device (F-AUT-006, chain-everywhere), so a kind cell is the same job with another candidate:
  the cut composed with that kind at the window's middle, the generator event composed with that kind
  at the cursor.

## Pros

- One look for the three kind pickers (rule 17): a cell is what the click gives, on the person's frame.
- No new service, no new decode path: the effect cells' lane, its misclick guard and its cache.

## Cons

- A still at the window's middle does not show the motion of the transition (the direction of a wipe
  reads from the shape, not from movement).
- A sound generator has no picture: its cell is its name.

## What users say

NO QUOTES: no third-party complaint was searched for this pair of pickers; the owner's own words are
the signal (`v26.10` line 17, quoted above).

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Both kind pickers are galleries of `fx_thumbs::kind_cell`: a `Swap` candidate on the effect cells' service, a transition at its window's middle, a generator on the event at the cursor.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `fx_thumbs.rs`, `gen_ui.rs`, `transition_editor.rs` |
| Project format | does not touch |
| Tests that will have to be written | a kind cell is the frame after the click (generator, transition); the pickers ask cells and pick by click |
| Depends on | F-AUT-006 |
| Breaks compatibility | no |

## Verdict

Take: the two kind pickers become galleries of the same cells as the effects picker — a transition
kind at the middle of its window over the real cut, a generator kind on the event at the cursor.

## Sources

- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (Transitions and Media Generators windows, access date 2026-11-18)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Effects Library, Hover Scrub Preview, access date 2026-11-18)
- https://support.apple.com/guide/final-cut-pro/ (Transitions browser, Generators browser, access date 2026-11-18)
