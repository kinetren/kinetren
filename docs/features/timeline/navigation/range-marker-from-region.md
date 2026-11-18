---
id: F-TL-016
title: A range marker from the region — a new one, or the existing one stretched
status: done
priority: C
cost: S
category: timeline/navigation
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [16, 17, 18, 20]
round: features-slow
point: change marker-region-split
tests: [t3104]
depends_on: []
trackers: []
services: []
checked: 2026-11-18
---

# F-TL-016 — A range marker from the region — a new one, or the existing one stretched

## Essence

The region (I/O) turned into a named range marker is how a person keeps "the chorus", "take 2" on the
ruler. Our "Marker over the region" did three things under one name: made a new range, stretched a
point marker that stood at the region's start, or removed an equal range. The owner found the
stretching useful but surprising under that name and asked for two items: one that always makes a
new marker, one that stretches the existing one (`v26.10` line 33, answered 2026-11-07).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Insert Region (R) | a region is its own object, made from the time selection; a marker is a point; a new region never changes a marker |
| Premiere | 26 | Add Marker + duration in the Marker dialog / Alt-drag | a marker gets a duration by editing it; making a marker over In/Out is a separate action from changing one |
| Resolve | 21 | duration marker (Alt-drag a marker; marker over In/Out) | the duration is edited on the existing marker; adding one where one exists edits that one |
| Final Cut Pro | 12 | markers are points; a range is a keyword or a range selection | no range marker |
| After Effects | 26 | composition marker with Duration | the duration is a field of the marker; a new one is added by a separate command |

## The trick behind it

- The editors that have range markers keep "make one" and "change one" as two acts: a new marker from
  the range is a command, a change of its length is an edit of that marker (a dialog field, an
  Alt-drag). Mixing them under one menu name is what made ours surprising.
- Vegas makes the distinction by type: a REGION (with length) and a MARKER (a point) are different
  objects with different keys (R and M), so a new region can never alter a marker.

## Pros

- A name says one result (rule 18): "Marker over the region" always adds, "Stretch the marker over the
  region" always changes.

## Cons

- One marker per tick (the model's rule): a new range cannot stand where a different marker already
  starts; the item then refuses and names the stretch item instead of creating a second one.

## What users say

NO QUOTES: the item's behaviour is ours; the owner's own words are the signal (`v26.10` line 33: "он
почему-то меняет уже выделенный регион, это полезно, надо вынести в отдельный пункт").

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The region-edge menu: "Marker over the region" (`marker_over_region_new`) always new, refused with the other item's name over a different marker; "Stretch the marker over the region" (`stretch_marker_over_region`) only when there is one; M unchanged.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `timeline.rs`, `app.rs` |
| Project format | does not touch |
| Tests that will have to be written | the region-edge menu: new, refusal, stretch, toggle |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: the split as the owner decided — "Marker over the region" always new (an equal range toggles off,
a different marker at the start is refused with the name of the other item), "Stretch the marker over
the region" only when there is one to stretch.

## Sources

- https://helpx.adobe.com/premiere-pro/using/working-with-markers.html (access date 2026-11-18)
- https://helpx.adobe.com/after-effects/using/layer-markers-composition-markers.html (access date 2026-11-18)
