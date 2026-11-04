---
id: F-TL-013
title: What Copy takes — the selected event or the time range; a range over an event by a gesture
status: done
priority: B
cost: S
category: timeline/assembly
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [16, 17]
round: ktprompts-26.10.18
point: change selection-copy-target (items 4–5)
tests: [924, 925, 926, 927, 928, 929]
depends_on: []
checked: 2026-10-28
---

# F-TL-013 — What Copy takes — the selected event or the time range; a range over an event by a gesture

## Essence

The editor has a time range on the ruler (a loop, a render range) and clicks one event to copy
it somewhere else. Ctrl+C must take what the person points at. Two sources of "what" live on
the timeline at once: the SELECTED objects and the TIME RANGE; every editor needs a rule which
one Copy takes when both exist. The second half is the reverse gesture: turning one event's
bounds into the time range in one move, without dragging the ruler edges to the event's edges by eye.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Copy (`Ctrl+C`) with a time selection; double-click an event | Copy works on the SELECTED events; a time selection limits them: selected events that cross the selection edges are split at the edges by Cut (the 2026 help, "Cutting, copying, and pasting events"); a time selection with NO selected event copies nothing of the events. A double-click on an event makes a time selection over the event's length — the gesture we put on Shift+double-click |
| Adobe Premiere | 26 | Copy; Mark Clip (`X`) | Copy takes the selected clips only, In/Out do not take part in Copy (they drive Lift/Extract). `X` sets In/Out to the bounds of the clip under the playhead on the targeted tracks [NOT VERIFIED against the 26 manual, from working knowledge of the 2024–2025 versions] |
| DaVinci Resolve | 21 | Copy; Mark Clip (`X`) | Copy takes what is selected (red/orange outline); with "Selection Follows Playhead" on, what is selected changes as the playhead moves, and forum threads blame exactly this for "Copy took the wrong clip". `X` marks In/Out over the clip under the playhead. Whether In/Out with nothing selected copies the range changed between versions (forum reports of DR16 and of 2024 disagree) [NOT VERIFIED against the manual] |
| Final Cut Pro | 12.3 | range selection (`R`), Set Clip Range (`X`) | there is no conflict by construction: a range selection IS the selection, so a clip selection and a range cannot both be active; `X` makes a range over the clip under the skimmer |
| After Effects | 26 | Copy of layers; work area | Copy takes the selected layers or keys; the work area never takes part in Copy. "Trim Comp to Work Area" goes the other way. A gesture "work area over this layer" exists as `Ctrl+Alt+B` (Work Area to selected layers) [NOT VERIFIED] |

## The trick behind it

**1. Four of five editors answer "the selection wins, the range only narrows or is ignored".**
Nobody lets a range silently override an explicit selection. Vegas, the only one where the range
takes part in Copy, uses it as a LIMIT on the selected events, not as a replacement for them —
so a click on one event and Ctrl+C never takes the neighbours on other tracks.

**2. "Range over this clip" is a separate one-key verb in every editor that has ranges** (`X`
in Premiere, Resolve and FCP, double-click in Vegas): its value is that the edges land exactly on
the clip's edges, which the eye cannot do at a deep zoom. It is offered on the clip under the
playhead (keyboard) or the clip clicked (Vegas), never as a menu-only action.

**3. A triple click as a gesture has a cost**: the double click fires on the second click before
the third is known, so in every toolkit a triple click arrives AFTER the double-click action. An
editor that binds both must make the double-click action harmless to undo or to repeat
(Vegas double-click = range, harmless; ours = opens the FX window).

## Pros

- Selection-first Copy matches what the person sees highlighted; a loop range kept for listening
  does not change what a click-and-copy takes.
- A one-gesture range over an event gives frame-exact edges for loop, render and region copy.

## Cons

- A rule with exceptions (one event vs several) is harder to predict than "the selection always
  wins"; the latter changes the range-marker gesture, which selects events AND sets a range.
- A triple click inherits the side effect of the double click (an FX window that opens and has
  to be closed again).

## What users say

NO QUOTES: searched (2026-10-28) the VEGAS forum and the Blackmagic forum for "copy selected
events time selection" and "copy in out range no clips selected"; found threads about grouping
and "Selection Follows Playhead" changing what Copy takes, but no short first-hand sentence about
selection-versus-range precedence that could be quoted verbatim within the rule's limits.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Copy takes one selected event whole, else the region, else the selection (`copy_selection`); a triple click on an event sets the region through `region_from_event`, the one body shared with Shift+double-click and the menu item.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app (`copy_selection`, timeline click handling) |
| Project format | does not touch |
| Tests that will have to be written | harness: copy with region + one / several selected; triple click |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: the owner asked for both (ktprompts 26.10.18 items 4–5); one selected event wins over the
region, a triple click sets the region over the event through the existing one body. The rule for
several selected events is decided in change `selection-copy-target` (archived 2026-11-04): they keep the
region rule, because "any selection wins" would change the range-marker double click; the target is named in the status line.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/cutting_copying_and_pasting_events.htm (access date 2026-10-28)
- https://www.vegascreativesoftware.info/us/forum/cntrl-c-and-v-of-events-with-groups--149766 (access date 2026-10-28)
- https://forum.blackmagicdesign.com/viewtopic.php?p=1097476 (access date 2026-10-28)
- https://creativecow.net/?p=1984736 (access date 2026-10-28)
