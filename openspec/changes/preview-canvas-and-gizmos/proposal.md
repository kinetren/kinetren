# Proposal: preview-canvas-and-gizmos

> DESIGN ONLY, revision 2 (2026-11-24, base `0c2a4065`): made implementation-ready against today's code (design D0).
> Registry `kkt-26.11.01b-index.md` B9. Tests: t3597–t3599, t4229–t4231. No open question.

## Why

The owner's items, whole:

- `kkt-26.11.01b.txt` line 10 (B9): "когда пытаешься в превью пправить зум панорамирования - нет возможности отзумиться в
  превью чтобы поправить большой зум, или задать поворот - вообщем нет нормальнго управления эффектом в превью, выглядит
  дешево"
- `kkt-26.11.20.txt` line 34 (A26, the answer to Q53 and Q54): "Контуры; не запоминать (Recommended)"

**The reading, checked.** The item names three things. Zooming the preview OUT to reach a large zoom's handles, and the
rotation, which exists since R.64 but is off the picture at such a zoom: one mechanism, the handles live only inside the
fitted picture. And "нет нормального управления эффектом в превью": effects have no handles beyond one crosshair pair.
His example is the pan's zoom; the class is every control drawn over the picture (D6). A26: outside the frame
outlines and handles only; the zoom is not remembered. Both read as designed.

## What Changes

- **Zoom and pan of the preview**: Fit (default) or 25, 33, 50, 100, 200, 400 %; Ctrl+wheel around the pointer, a menu in
  the corner, the wheel and Shift+wheel pan when not at Fit, a middle drag pans. The composite is unchanged: the display
  draws the presented picture scaled.
- **The pasteboard**: the place around the canvas is neutral; the transform frame with its rotation handle, masks,
  centres and effect handles are drawn and grabbed there.
- **Effect handles by declaration**: a plugin declares what its rows are on the frame — a centre, a radius, an angle, a
  quad — and the preview draws and edits them, for an event's slot, a morpher's and a track's.

## Capabilities

### Modified Capabilities

- `ui-timeline`: the viewer transform and the pasteboard.
- `ui-reach`: effect handles by declared kinds.
- `ui-controls`: the command "Fit the preview".

## Impact

- kt-plugin-api: `UiDecl.handles: &[HandleDecl]`; declarations in `fx-video-{color,distort,blur}`.
- kt-app: `viewer.rs` (new), `app.rs` (`draw_preview`), `geomctl.rs`, `maskctl.rs`, `centerctl.rs` → `handlectl.rs`,
  `controls.rs` (one command); i18n ×5.
- No model, engine or format change; nothing stored. OTIO: nothing. Idle: zoom and pan redraw the display only.

## Questions to the owner

None. Q53–Q54 as asked are in the git history of this file (ffe4eebf).
