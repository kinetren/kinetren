---
id: F-UI-009
title: An effect's centre dragged on the preview, its X and Y on one row
status: done
priority: B
cost: S
category: ui/viewer
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [7, 12]
ui_rules: [1, 3, 10, 17, 25]
round: features-slow
point: change fx-center-on-preview
tests: [t3123, t3124, t3125, t3126]
depends_on: []
trackers: []
services: []
checked: 2026-11-19
---

# F-UI-009 — An effect's centre dragged on the preview, its X and Y on one row

## Essence

Radial Blur's centre was two number fields on different rows, set by guessing. The owner (`v26.10`
line 52, row T10): "Radial Blur имеет параметры центра X Y, и эти параметр находятся на разных
строках. Мало того - я еще не могу как в том же Vegas указать центр по самой превью. А Vegas - это
базовый минимум".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | effect centre handles in the Video Preview | an effect with a centre (Radial Blur, Pinch/Punch, Spherize) shows a crosshair on the preview while its dialog is open; dragging it sets the centre |
| Premiere | 26 | direct manipulation in the Program Monitor | selecting an effect in Effect Controls shows its point handles (centre, anchor) on the monitor; drag them |
| Resolve | 21 | on-screen controls (Viewer) | Fusion/OpenFX effects show their centre widgets on the viewer when "on-screen controls" are on |
| Final Cut Pro | 12 | onscreen controls | an effect with a centre shows a draggable control on the viewer while selected |
| After Effects | 26 | effect control points | every Point property of the selected effect is a crosshair on the composition, draggable; keyframed — the key at the current time |

## The trick behind it

- The handle belongs to the SELECTED effect only: a frame full of every effect's points is noise.
- The point is in the frame the effect works on: before a transform it is the source, after it the
  canvas — the same map the effect's own masks use.
- A point with keyframes edits the key at the current time, as the field does (AE).

## Pros

- The centre is placed where the person looks; the number follows.

## Cons

- Only effects that HAVE a centre parameter show it: Twirl, Lens Distort, Ripple and Kaleidoscope
  have their centre fixed at the frame's middle in their kernels — a centre for them is a kernel
  change of its own (named in the class walk, not done here).

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own words (quoted above) are the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/ — the contract is in `spec.md`, the "why" in `design.md`.
`centerctl.rs`: the selected slot of an open effects window with a `center_x`/`center_y` pair shows a
crosshair over the preview (paused), mapped through `maskctl::slot_frame_view`; a drag writes the
base, or the key at the playhead when the pair has a curve, in one history step. `params_grid` keeps
every `_x`/`_y` pair on one row.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `centerctl.rs`, `maskctl.rs`, `widgets.rs` |
| Project format | does not touch |
| Tests that will have to be written | the crosshair at the centre, a drag writes the pair as one step; with a curve the key; hidden while playing and for effects without a centre; the pair on one row |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: a crosshair for the selected effect's centre on the preview, the pair on one row.

## Sources

- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (video effects, preview handles, access date 2026-11-19)
- https://helpx.adobe.com/premiere/desktop/ (direct manipulation in the Program Monitor, access date 2026-11-19)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (on-screen controls, access date 2026-11-19)
- https://helpx.adobe.com/after-effects/ (effect control points, access date 2026-11-19)
