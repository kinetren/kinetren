---
id: F-UI-011
title: Zooming the preview past the frame, and handles for every effect that has a place on it
status: candidate
priority: B
cost: M
category: ui/viewer
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-plugin-api]
invariants: [7, 12]
ui_rules: [1, 13, 17]
round: null
point: null
tests: []
depends_on: [F-UI-001, F-UI-009]
checked: 2026-11-20
---

# F-UI-011 — Zooming the preview past the frame, and handles for every effect that has a place on it

## Essence

A pan zoomed to 3× has its frame and handles three times larger than the picture: they lie outside the
preview, and nothing can be grabbed. Every compositor lets the viewer zoom OUT (25 %, 50 %) so that the canvas
is small and the space around it — the pasteboard — holds the off-frame outlines and handles; and zoom IN to
place a point precisely. The second half: effects with a place on the frame (a centre, a radius, an angle, four
corners) are edited by handles on the picture, not by numbers.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Pan/Crop window's own zoom; preview "Zoom" in the Video Preview | the Pan/Crop window shows the frame in a workspace you can zoom out of, so an oversized frame is reachable [NOT VERIFIED live] |
| Adobe Premiere | 26 | Program Monitor zoom level (10 %…400 %, Fit) | at 25 % the gray pasteboard shows the bounding boxes of clips outside the frame; handles are reachable there [NOT VERIFIED live] |
| DaVinci Resolve | 21 | Viewer zoom; Transform/Crop on-screen controls; Fusion viewer | zoom out shows off-frame controls; OFX effects expose on-screen controls [NOT VERIFIED live] |
| Final Cut Pro | 12 | Viewer zoom (Fit, 12.5–400 %) and on-screen controls of effects | effects declare on-screen controls (centre, radius, angle) [NOT VERIFIED live] |
| After Effects | 26 | Composition viewer magnification; effect point controls | the pasteboard shows layer bounds outside the comp; every effect "point" parameter is a crosshair on the viewer [NOT VERIFIED live] |

## The trick behind it

**1. The pasteboard draws outlines, not pictures.** Outside the composition, compositors draw the layer's
bounding box and handles, not the layer's pixels — the composite is still the frame's size, so zooming out
costs nothing in rendering. Only the overlay grows.

**2. Effect handles come from the parameter's TYPE, not from per-effect code.** AE makes every "point"
parameter a crosshair automatically; an angle parameter gets a dial around its point; a radius a circle. An
effect author declares types, the viewer draws the handles — so every new effect is controllable on screen
without new UI code.

## Pros

- Large zooms and off-frame positions become editable; precise placement at 200–400 %.
- Every effect with a place on the frame gets handles by declaration.

## Cons

- Two zooms (the timeline's and the viewer's) — the viewer's must be visibly separate and reset by one gesture.

## What users say

The owner, `docs/ktprompts/kkt-26.11.01b.txt` line 10: "когда пытаешься в превью пправить зум панорамирования -
нет возможности отзумиться в превью чтобы поправить большой зум, или задать поворот - вообщем нет нормальнго
управления эффектом в превью, выглядит дешево".

NO QUOTES: no web access in this spec session (2026-11-20) for outside quotes; editor rows [NOT VERIFIED].

## How it fits Kinotrain

### What already exists

- The transform frame of the Geometry node on the preview (R.64, F-UI-001, capability `ui-timeline`):
  position, zoom, rotation handle, anchor, crop; small-frame handles outside the edges
  (`src/apps/kt-app/src/geomctl.rs`).
- Effect centres as crosshairs (F-UI-009, `src/apps/kt-app/src/centerctl.rs`): every `_x`/`_y` pair.
- The preview texture drawn fitted to its area (`src/apps/kt-app/src/preview_display.rs`).

### What is missing

- A viewer zoom/pan; a pasteboard; handle types beyond the centre (radius, angle, quad).

### What hinders

- The rotation handle exists, but at large zooms it is off the visible canvas — the complaint's "или задать
  поворот" is the reachability problem, not a missing handle.

### Which invariants it touches

- **7** — the composite is unchanged (the canvas size); only the overlay and the display scale move.
- **12** — zoom/pan redraw the display texture scaled, never re-compose.
- CAPABILITY: openspec/specs/ui-timeline/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | kt-app (viewer transform, pasteboard overlay, handle kinds), kt-plugin-api (a handle declaration on rows) |
| Project format | none (the viewer zoom is UI state) |
| Tests | handles of a 3× zoomed frame reachable at 33 %; zoom never recomposes; a radius handle edits the row |
| Depends on | F-UI-001, F-UI-009 (done) |
| Breaks compatibility | no |

## Verdict

Take it: a viewer zoom (Fit, 25–400 %) and pan with a pasteboard of outlines and handles, and handle kinds
declared on passport rows (point exists; radius, angle, quad added) so every effect gets on-screen control.

Design `preview-canvas-and-gizmos` revision 2 (2026-11-24): Space stays play, Ctrl+0 stays the interface zoom, Fit is Shift+0;
handles are declared on the six built-in effects whose rows are places on the frame (the note's "every `_x`/`_y` pair" is
today one pair, Radial Blur's).

## Sources

- `docs/ktprompts/kkt-26.11.01b.txt` line 10; `docs/user-feedback.md` "the second list" (B9 taken: "zoom and
  pan the preview canvas past the frame, a rotation handle on the on-screen frame, and on-screen handles for
  effects that have a place on the frame").
