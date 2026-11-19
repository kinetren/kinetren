---
id: F-MED-009
title: A bin card dragged to the timeline lands with the scrubbed moment under the mouse
status: done
priority: B
cost: S
category: media/organization
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [8, 12]
ui_rules: [1, 4, 10, 16, 17]
round: features-slow
point: change bin-drag-moment
tests: [t3117, t3118, t3119]
depends_on: [F-MED-008]
trackers: []
services: []
checked: 2026-11-19
---

# F-MED-009 — A bin card dragged to the timeline lands with the scrubbed moment under the mouse

## Essence

Scrubbing a card in the bin finds the moment a person wants (F-MED-008). Dragging the card to the
timeline then placed the file with its START at the mouse, so the found moment ended up wherever the
file's length put it. The owner (`v26.10` line 156, MW1): "как-то все таки обыграть перенос на таймлайн
чтобы видео встало в нужную точку сразу"; asked whether the event should be cut at the moment, he
answered (kkt-26.11.18b U9): "целиком, момент под мышью. а если прожали alt то начало события будет под
курсором, может быть еще какую-то комбинацию чтобы конец был под курсором … до момента отпускания зажать
альт … чтобы уже визуально до вставки была голограма события".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Trimmer; Project Media | a clip dragged from Project Media lands from its start at the mouse; a moment is aimed by marking in/out in the Trimmer and dragging from there — the selection, not the whole file |
| Premiere | 26 | Project panel Hover Scrub, Source Monitor | a drag from the Project panel lands the clip from its In point; a moment under the mouse is not kept — aiming goes through In/Out marks in the Source Monitor |
| Resolve | 21 | Media Pool, Source Viewer | the same: whole clip from its start, or the In–Out range of the source viewer; the timeline shows the clip's outline while dragging |
| Final Cut Pro | 12 | Browser skimming | skimming picks a frame, a range is selected by dragging across the filmstrip; dropping a clip lands its start (or the range's) at the pointer |
| After Effects | 26 | Project panel | layers land at the current time indicator; no scrub-to-moment drag |

## The trick behind it

- Every editor makes the person choose a RANGE first (in/out) and drops the range. None keeps "the
  moment I was looking at" as an anchor of a whole-file drop: the hover is thrown away at the press.
- The press point on the scrub strip already IS that moment: the strip maps x to the file's time,
  so the anchor costs nothing — one number carried with the drag.
- An anchor that is not the start needs the drop to SHOW where the event goes before the release:
  the ghost has to be the event's real length, not a fixed mark.

## Pros

- One gesture for the commonest aim: "this line of dialogue under that beat" without marking in/out.
- Alt / Alt+Shift give the two other anchors (start, end) without leaving the drag; the ghost and the
  status line say which one holds.

## Cons

- The whole file lands: a part before the moment is still there to trim (his choice: "целиком").
- Alt+drag starting with the button is the window manager's move on many Linux desktops; Alt pressed
  AFTER the drag started is the program's.

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own words (quoted above) are the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" in `design.md`.
The bin card's press on the scrub strip records the file moment (`KtApp::bin_press_moment`), the drag
payload `BinDrag(media, moment, length)` carries it, and `timeline::BinLanding` reads Alt/Shift every
frame: the ghost is the event's real length at its real start, a white tick marks the point under
the mouse.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `bin_panel.rs`, `timeline.rs` |
| Project format | does not touch |
| Tests that will have to be written | the moment lands under the mouse; Alt and Alt+Shift move the landing live; the ghost is the file's length |
| Depends on | F-MED-008 |
| Breaks compatibility | no |

## Verdict

Take, as he answered: the whole file, the scrubbed moment under the mouse; Alt — the start, Alt+Shift
— the end; the modifier read live, the ghost the real event.

## Sources

- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (Trimmer, Project Media, access date 2026-11-19)
- https://helpx.adobe.com/premiere/desktop/ (Project panel, Source Monitor, access date 2026-11-19)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Media Pool, Source Viewer, access date 2026-11-19)
- https://support.apple.com/guide/final-cut-pro/ (skimming, adding clips, access date 2026-11-19)
