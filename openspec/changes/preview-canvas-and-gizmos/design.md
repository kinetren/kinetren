# Design: preview-canvas-and-gizmos

> DESIGN ONLY, revision 2 (2026-11-24), base `0c2a4065`.

## D0. What revision 1 said and the code contradicts

1. **Space+drag pans.** Space is play/pause and its hold is the speed-up (R.72), in every layout. Dropped.
2. **Ctrl+0 = Fit.** Ctrl+= / Ctrl+- / Ctrl+0 are egui's interface zoom, kept free on purpose (`controls.rs:915`
   `ui_zoom_keys_free`). Fit is a command of its own with Shift+0.
3. **Twirl, Ripple and Lens with a centre and a radius.** They have no such rows: twirl is `angle`, `radius` around the
   frame's centre; ripple `amount`, `freq`, `phase`; lens `k1`, `k2`. Only rows that exist get handles; a centre for
   them would be a new row and a new picture rule (not asked; `geometry-fx-flex` may).
4. **"Every `_x`/`_y` pair is a crosshair."** Only `center_x`/`center_y` (`centerctl::PAIR`), that is Radial Blur alone.
5. **A field on `ParamSpec`.** 31 literals build it; a handle joins several rows. It is a list on `UiDecl` (which has a
   `Default`).

## D1. The viewer (`apps/kt-app/src/viewer.rs`)

`Viewer2d { zoom: Zoom, pan: egui::Vec2 }`, `Zoom::{Fit, Pct(u16)}`; a field of `KtApp`, reset by opening a project,
not stored (A26). `canvas_rect(rect, raster, ppp) -> Rect`:
- Fit: today's arithmetic (`app.rs:7770`), bit for bit;
- `Pct(z)`: one PROJECT pixel takes `z/100` physical pixels (`settings.width × z/100 / pixels_per_point`), whatever the
  preview's quality; centred, moved by `pan`, clamped so that 10 % of the canvas stays in `rect`.
It is called once, in `draw_preview`; its result is `img_rect`, which the frame, masks, centres, title and the corner
button already take. Its clamp runs each frame, so a resized or detached preview stays sane.

**Gestures**, read in `draw_preview` while the pointer is over `rect`: `zoom_delta` (Ctrl+wheel, a pinch) goes to the next
step of the list around the pointer (the point under it stays), one step per 10 % of delta accumulated; a plain wheel
pans vertically and Shift+wheel horizontally when not at Fit (at Fit the wheel does nothing, as today); a middle drag
pans. A click stays play/pause, a double click detach, also on the pasteboard.

**The corner menu**: a small button at the lower right of the preview's rectangle, "Fit" or "33 %", drawn while the pointer
is over the preview or the zoom is not Fit; its menu lists the seven. Probe `preview.zoom`. Command `Cmd::PreviewFit = 38`
(frozen, append only; `CMD_COUNT` 39) in group View, Shift+0 in every style, in the key list and the settings code.

**Drawing**: at Fit exactly as today (glow, vignette, outline). Not at Fit: `rect` filled with grey 38, no glow, the
picture at `img_rect` through the painter clipped to `rect`, a thin outline. Filter: nearest at 200 and 400 %, linear
below (a sampler of the display texture's registration; the bytes path sets `TextureOptions`). What is shown at 400 % with
the preview at ½ is the ½ composite enlarged: the zoom asks no composite (A26's "outlines only" has no other reading of
the inside).

## D2. The pasteboard

`painter_at(rect)` already clips to the preview; what is missing:
- `maskctl.rs:638`: the gesture zone `.intersect(img.expand(GRAB))` becomes `.intersect(rect)`; with a drawing tool armed
  the zone is `rect` (a mask may start outside the picture: its coordinates are not clamped to it);
- `geomctl` zones are not clipped by `img` (`:393`); egui gives the pointer only where `rect` is, and `interact` is
  clipped by the ui's clip: the call passes `body.intersect(rect)`;
- `geom_toggle_button` stands at the corner of `img`: at zoom its place is the corner of `img ∩ rect`, and Fit's otherwise;
- `title_drag_in_preview` lays the title out at `img`'s size every frame (`app.rs:5033`): at 400 % that is a 4× layout per
  frame; it lays out at the Fit size once and scales the block; `.intersect(img)` becomes `rect`;
- safe zones, the Viewer's badge and Loading stand over `img ∩ rect`;
- a frame wholly outside `rect` after a pan: the corner menu's "Fit" and the clamp (10 %) bring it back.
The outline of the selected event is its placement quad, as drawn today; nothing of its picture is composed there.

## D3. Handles by declaration (kt-plugin-api)

```text
pub enum HandleDecl {
    Centre { x: &'static str, y: &'static str },                      // −1..1 across the frame
    Radius { row: &'static str, at: At, unit: RadiusUnit, from: Option<&'static str> },
    Angle  { row: &'static str, at: At, zero: f64 /* deg, screen clockwise */ },
    Quad   { rows: [[&'static str; 2]; 4] },                          // 0..1 of the canvas
}
pub enum At { FrameCentre, Centre }                                   // Centre: the slot's Centre decl
pub enum RadiusUnit { ShortSemiAxis, Frame { kx: f64, ky: f64 } }     // px = v·min(w,h)/2, or an ellipse v·k·(w,h)
```
`register` refuses a declaration naming a row that is not a smooth number of the passport, or `At::Centre` without one.

Declared now (every built-in row that is a place on the frame, walked over the passports):

| Plugin | Handles |
|---|---|
| radial_blur | Centre(center_x, center_y) |
| twirl | Radius(radius, FrameCentre, ShortSemiAxis); Angle(angle) |
| vignette | Radius(radius, FrameCentre, Frame 0.7071); Radius(softness, the same, from radius) |
| kaleidoscope | Angle(angle) |
| corner_pin | Quad(x0 y0, x1 y1, x2 y2, x3 y3) |
| halftone | Angle(angle) |

Not declared, with the reason: drop_shadow (its offset is in pixels of the composing raster, not of the canvas: a
handle would move with the quality), lens, ripple, cylinder, pixelate (no row is a place), pan3d (a 3D handle is
`real-3d-groups`' work), Camera ISP, VHS, Datamosh (a device's panel). `geometry-fx-flex` adds Mirror's line and Tile's
quad by the same kinds.

## D4. The controller (`handlectl.rs`, from `centerctl.rs`)

- **The owner**: `HandleOwner::{Event(clip, slot), Patch(morpher, slot), Track(track, slot)}`: the selected slot of an open
  effects, morpher or track window (first the event's, as `center_target` today). The frame: an event's slot by
  `slot_frame_view` (the source before the Geometry node, the canvas after); a patch's and a track's slots see the
  canvas (a patch is appended after the event's chain).
- **Values and writes** as `center_value` / `write_center` do: the key at the playhead when the row has a curve, else the
  base; `SetFx`/`SetFxAuto`, `SetMorpher`, `SetTrackFx`; coalesced by the gesture; only a real change is sent. A track's
  row has no curves.
- **A drag**: from the press (geomctl's rule: the press point and the values at the press). Radius: the distance from the
  centre along the frame's axes; Angle: the turn accumulates past 360° to the row's range (twirl ±720), Shift snaps 15°;
  Quad: the corner under the press.
- Handles show on pause only, never during playback; `GRAB` zones; nearest first; a smaller target wins (registered after
  the frame, as `center_controls` is). Hints name the row and its value. Probes `fx.handle.<kind>.<row>`.
- At Fit a handle outside `rect` is not reachable: the hint of the slot says "zoom the preview out (Ctrl+wheel)".

## D5. Cost

Zoom and pan: a redraw of one texture and the overlay, no composite, no decode, no compose request (the preview's key is
the moment and the raster, not the rectangle; t3599 counts). Handles: a mesh per frame while a window's slot is selected
on a pause; nothing when the program rests.

## D6. Class walk — "a control drawn in a space the person cannot reach"

| Control | Today | Here |
|---|---|---|
| transform frame, rotation, anchor, crop | inside the fitted picture's surroundings | pasteboard |
| masks: vertices, feather, rotation, new shapes | zone cut to the picture | pasteboard, drawing too |
| effect centre | one pair, events only | declared kinds, three owners |
| title drag | frame cut to the picture | pasteboard |
| the wand's seeds (`magic-wand-mask`), tracker boxes (`point-tracker`) | — | they use `View`/`img_rect`, so the viewer's |
| the transform form's schema (fx window) | fits any zoom since R.67 | unchanged |
| the graph canvas, curves, timeline | have their own zoom | unchanged |
| the camera's look-at, 3D rotation | a form | `real-3d-groups` |
| formats (`frame-format-variants`) | one raster | `canvas_rect` takes the raster shown |
| the detached preview | the same `draw_preview` | the same |

## Rejected

- A padded composite for the pasteboard (A26). - Space+drag, Ctrl+0 (D0). - A stored zoom (A26). - Continuous zoom: a list of
  steps is exact and comes back by the same wheel.
