# Design: preview-canvas-and-gizmos

> DESIGN ONLY, revision 1 (2026-11-20), base `756f41b5`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- The transform frame (R.64, `ui-timeline` "Transform frame over the preview picture"): one placement function
  shared with the render's geo-program; handles outside a small frame; the rotation handle; one history step per
  gesture (`src/apps/kt-app/src/geomctl.rs`).
- Effect centres (F-UI-009, `centerctl.rs`): every `_x`/`_y` pair is a crosshair.
- The display: `preview_display.rs` registers the engine's display target; the picture is drawn fitted into the
  preview rect.
- Controls: keys only from `controls.rs` (t1715).

## D1. The viewer transform

`Viewer { zoom: Zoom::{Fit, Pct(u16)}, pan: Vec2 }` (UI state of the preview, not the project). Canvas rect on
screen = fit rect × zoom / fit scale, offset by pan, clamped so at least 10 % of the canvas stays visible. One
function `viewer.canvas_rect(preview_rect, raster)` is the ONLY source of the canvas rectangle for the picture,
the transform frame, the crosshairs and the new handles — the gizmos stop computing from the fitted rect.
Ctrl+wheel zooms around the pointer; middle-drag and Space+drag pan; Ctrl+0 Fit; a corner menu lists the steps.
The display texture is drawn scaled (nearest above 100 %, linear below — the existing filter switch).

## D2. The pasteboard

The preview rect outside the canvas is drawn as a neutral grey; gizmos and handles are clipped to the PREVIEW
rect, not the canvas; the selected event's off-canvas outline (its placement quad) is drawn as a thin line. No
composite change (Q1 a).

## D3. Handle kinds

`ParamSpec.handle` (kt-plugin-api), a UI declaration:
- `Point` — today's `_x`/`_y` pairs (implicit, unchanged);
- `Radius { centre: "<stem>" }` — a circle around the named point, dragging its rim sets the row (in the row's
  unit: normalized or px, through the same placement map);
- `Angle { centre }` — a dial line from the centre, dragging sets degrees (Shift snaps 15°);
- `Quad { corner: 0..3 }` on four pairs — a quad with draggable corners (Corner Pin).
Each gesture is one history step, edits keys when the row is curved (the frame's rule), and the number equals the
form field's. The handles of the SELECTED slot are drawn (as the centres today); the selected slot without
handles draws none.

Declared now: Vignette (radius), Twirl (centre, radius, angle), Ripple (centre, radius), Lens distort (centre),
Radial Blur (centre), Kaleidoscope (centre, angle), Corner Pin (quad), Mirror reflect (centre, angle — change
`geometry-fx-flex`), Tile (source rect as a quad — same change). A plugin without declarations is unchanged.

## D4. Class walk — "a control drawn in a space the person cannot reach"

| Control | Today | Here |
|---|---|---|
| transform frame handles | clipped to the canvas area | pasteboard |
| effect crosshairs | inside the canvas | pasteboard |
| mask vertices (shape-masks) | on the canvas | pasteboard (the same viewer transform) |
| tracker boxes (change `point-tracker`) | — | pasteboard |
| wand seeds (change `magic-wand-mask`) | — | the viewer transform |
| camera look-at points | in the camera form | out of scope (3D gizmo is its own work) |

## D5. Idle

Zoom and pan redraw the display texture scaled — no recomposition, no decode (invariant 12; t3599 counts no
composite and no decode request during a zoom and pan on a pause).

## Rejected

- **Rendering a padded canvas for the pasteboard by default** — a second composition size while editing (Q1 b
  is the owner's to choose).
- **Per-effect handle code** — every new effect would need UI code; declarations scale.
