# Proposal: preview-canvas-and-gizmos

> DESIGN ONLY (spec session, 2026-11-20). Registry `kkt-26.11.01b-index.md` B9 (`preview-canvas-and-gizmos`,
> queue 6). Tests reserved: t3597–t3599.

## Why

The owner, `kkt-26.11.01b` line 10: "когда пытаешься в превью пправить зум панорамирования - нет возможности
отзумиться в превью чтобы поправить большой зум, или задать поворот - вообщем нет нормальнго управления
эффектом в превью, выглядит дешево". Taken (2026-11-01): "zoom and pan the preview canvas past the frame, a
rotation handle on the on-screen frame, and on-screen handles for effects that have a place on the frame."
The rotation handle exists since R.64, but a zoomed frame puts it — and every other handle — off the visible
canvas. Note F-UI-011.

## What Changes

- **Viewer zoom and pan**: the preview shows the canvas at Fit (default) or 25 %, 33 %, 50 %, 100 %, 200 %, 400 %
  (a menu in the preview's corner, Ctrl+wheel over the preview, Ctrl+0 = Fit); a middle-drag or Space+drag pans.
  The composite is unchanged — the display scales the presented texture.
- **A pasteboard**: around the canvas, a neutral area where the transform frame, its handles and every effect
  handle are drawn and grabbed even when they lie outside the canvas; the off-canvas part of a layer is drawn as
  its outline (Q1).
- **Handle kinds declared on rows**: besides the centre crosshair (point pairs), a passport row may declare
  itself a radius (a circle around a named centre), an angle (a dial around a named centre) or a corner of a quad
  (four point pairs → a draggable quad). The viewer draws handles from these declarations; Vignette, Twirl,
  Ripple, Lens, Radial Blur, Corner Pin, Mirror (reflect), Tile and Kaleidoscope get handles without their own
  UI code.
- **The rotation handle at any zoom**: reachable on the pasteboard; it stays the same gesture.

## Capabilities

### Modified Capabilities

- `ui-timeline`: the viewer zoom/pan and the pasteboard; the transform frame and effect handles on it.
- `ui-reach`: the zoom menu and keys; handle kinds reach every effect that declares them.

## Impact

- kt-plugin-api: `ParamSpec.handle: Option<Handle>` (`Radius { centre }`, `Angle { centre }`, `Quad { corner }`),
  declared in the plugins' passports (no file change: a UI declaration).
- kt-app: the viewer transform (zoom, pan) shared by the frame, the centre crosshairs and the new handles
  (`geomctl.rs`, `centerctl.rs` take the viewer transform instead of the fitted rect); the pasteboard overlay;
  `controls.rs` rows (Ctrl+wheel, Ctrl+0 in every layout); i18n ×5.
- No model/engine/format change; `UiState.viewer_zoom` not stored (a view, resets to Fit on open — Q2).
- Idle: zoom/pan redraw the display only; handles are a mesh rebuilt per gesture.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20` A26, his Q53–Q54): Q1 (a), outlines and handles only outside the frame; Q2 (a),
the zoom is not remembered.

The questions as asked:

**Q1. Outside the frame: outlines or the picture?** Premiere/AE draw only outlines on the pasteboard; showing
the picture beyond the frame needs a larger composite while you edit.
- (a) **Outlines and handles only** (*recommended*: no extra render cost; the frame is what is exported).
- (b) The picture too, composed on a larger canvas while the viewer is zoomed out (costlier preview).

**Q2. Is the viewer zoom remembered?**
- (a) **No: every opening starts at Fit; Ctrl+0 returns to Fit** (*recommended*).
- (b) Remembered per project in its view state.
