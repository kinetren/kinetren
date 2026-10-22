# Design

## Context

See proposal.md — Why. Below is only what is already in the code and defines the shape of the solution.

- **The arithmetic of placing the leading node is already computed, and it is one — `src/libs/kt-fx/src/geom.rs`,
  the `FxInstance::Geometry` branch of the builder: `dest` from the conform rule (`conform_dest` by the kind of
  source, the crop cuts the kind BEFORE the rule), `dw = dest·zoom`, center `fw/2 + pos_x·fw`, anchor in fractions of the
  frame, rotation around the anchor, `bounds` — transparent margins. Everything is in FRACTIONS of the canvas, so the parameters do not depend on the preview fraction.
  Now this arithmetic lives INSIDE the builder and is not exposed outward.
- **The machinery of the live gesture is ready**: `KtApp::apply_coalesced(Command::SetGeometry {…}, gesture)`
  (`src/apps/kt-app/src/app.rs`) edits the model, raises `model_rev` and pushes the project into the player — exactly what
  the FX window already uses (`fx_editor.rs`, the line with `SetGeometry`). The whole session of a gesture with
  one token is ONE history step.
- **The preview key already sees the edits of the model**: `content_version` and `project_version` go into it
  (R.62, t722b). The defect "the recomposition gate does not look at the revision", recorded in the note
  F-UI-001 of 2026-09-07, has been FIXED — we do not take it in this pass.
- **The similarity defect is alive**: the preview fractions compute the axes independently (`(w·f) as u32 & !1`), and 1920×1080
  gives 240×134 at 1/8 and 120×66 at 1/16 — 1.791 and 1.818 against 1.778.
- **The canvas of the composite is the preview quality itself** (`compose_preview_into` returns `Quality`),
  and the picture is put on the screen at one scale: `img_rect = from_center_size(rect.center(), q·s)`
  (`draw_preview`). So the chain "screen → canvas → canvas fractions" is two multiplications without rotations.
- **The preview already catches clicks**: a single one — play/pause, a double one — detach the window, a right one — the menu
  (`draw_preview`). A new gesture must be built into this parsing, not next to it.
- **A program rule**: the handles are drawn at the SELECTED event ("a gesture that is not visible does not
  exist" — `apps/kt-app/CLAUDE.md`, the section about the top strip of the event).

## Goals / Non-Goals

**Goals:**

- One common layer of controls over the frame, computing the same geometry as the render.
- Five gestures (move, scale, rotate, anchor, crop) over the EXISTING commands and the
  existing undo mechanism — without new commands and without changes to the format.
- A form of the geometry fields that is not ashamed to be placed next to Premiere/AE.
- A preview similar to the project frame at all fractions.

**Non-Goals:**

- Cameras and `Pan3d` (they have their own fork "which camera is active" and their own units — world px).
- Masks, the keyframe path, guides (safe zones, grid, center), "before/after" comparison modes.
- Multi-selection: the frame belongs to ONE selected event (see the decision below).
- Bringing the form of the fields in ALL other windows into line: the owner said "in the future we will need to go over this in all other widgets and windows too" — this is separate work, here only the geometry.

## Decisions

- **The placement rectangle is moved into a public function of kt-fx, and the builder calls that same one.**
  `geom::lead_placement(lead: LeadCtx, canvas: (f64,f64), g: &EventGeometry) -> Placement`
  (`Placement { ox, oy, dw, dh, rot_deg, anchor_px, crop_view }`), and the `FxInstance::Geometry` branch
  builds its `Aff`/`Proj` FROM IT. The alternative "compute the frame in kt-app by the same formulas" was rejected: it is a second source of the same truth, and they would diverge silently — exactly the class
  of defect for which the rule `clip_slice` (one formula of the slice) appeared in R.39.
- **The inverse recomputations are a pair to the same function, in the same file**: `pos_for_drag`,
  `zoom_for_handle`, `rot_for_point`, `anchor_move`, `crop_for_handle`. All are pure functions without egui, tested on the CPU without GPU and without FFmpeg (the crate rule: kt-fx depends on neither the one nor the other).
- **Anchor: the compensation is computed BY THE RECTANGLE, not by the offset formula.** `anchor_move`
  returns `(anchor, pos)` such that `lead_placement` before and after gives the same rectangle and
  the same rotation. This is the only form that does not break on a rotated layer: After
  Effects derives the compensation analytically and it lies on rotation (quotes in F-UI-001). The guard —
  t761 compares the rectangles, not the values.
- **The gesture goes through the EXISTING command**: `Command::SetGeometry { id, geometry, pos, auto }` via `apply_coalesced` with a token issued on the press. No "live" fields bypassing the model: the picture at the tempo of the gesture is obtained because `apply_coalesced` pushes the project into the player (SetGeometry
  touches the plan). The alternative "the drag edits a copy and commits on release" was rejected: then
  the preview shows the old frame, and a control without a live picture is useless.
- **An automation curve under the gesture**: if the parameter HAS a curve, the gesture edits the KEY under the cursor
  (by the same rule as the field in the FX window — `CurveBind`), not the constant. There must be no second
  rule in the program for "where to write the value".
- **The frame belongs to ONE selected video event covering the cursor.** Zero selected, more than
  one, an audio event, an event not under the cursor — there is no frame, the reason is named by the status line
  (rule 16). The alternative "a frame around the union of selected ones" was rejected: the gesture would have to be split into N commands with different anchors and different conform rules — this is another feature
  (group transformation), and it cannot be decided on the owner's behalf.
- **The parsing of a click on the preview is supplemented, not duplicated.** Order: handle → body of the frame →
  the former parsing (play/pause, double click, menu). While a drag is in progress, a single click on the preview does not fire. The reverse is also true: with the controls off, the preview behaves
  exactly as today (the byte-for-byte former path for the t-preview tests).
- **The toggle for showing — in the preview itself and in its right-click menu**, the state — the viewport (`UiState`, `dirty_ui`), no new windows
  (rule 22 of R.63). During playback the controls hide THEMSELVES and return on pause — the industry behaviour (FCP always hides) and our rule of frame cost.
- **Similarity of the preview fraction — the nearest EVEN PAIR, not independent rounding of the axes.** For a fraction
  a pair (w, h) is sought from the neighbourhood ±2 along each axis, minimizing |w/h − pw/ph| (on a tie — the larger area).
  For 1920×1080: 1/8 → 238×134 (1.7761 instead of 1.7910), 1/16 → 118×66
  (1.7879 instead of 1.8182). Evenness remains a requirement of the decoder (4:2:0), therefore exact
  similarity at small fractions does not exist — and the requirement of the spec is formulated as "no more than
  half a pixel along the short side".
- **The source view is given to the interface by the player**: `Player::source_view(media) -> Option<(f64, f64)>`
  on top of the already existing `SourceFacts.views`. The alternative "ask kt-media directly from the UI" was rejected
  by invariant 5: the decoder is owned by the worker, the interface does not go to it.
- **The "Transformation" form** is assembled from the existing bricks of `widgets.rs` (`param_row`,
  slider, `pub_label` with the publication door) — exactly two new widgets: a row of the X/Y pair and
  the frame scheme. The folding of groups is the state of the FX window, not of the project.

## Risks / Trade-offs

- [The gesture edits the frame every few milliseconds, and the preview composite at 1/2 costs tens of ms] →
  the gesture does not wait for the composite: `apply_coalesced` pushes the project, and the preview is rebuilt at its
  own tempo; on a weak machine the picture lags behind the frame, but the frame always shows the MODEL.
- [The handles overlap the frame exactly where the person is looking] → hiding during playback, a toggle,
  and the outline is thin: there is no fill on top of the picture.
- [A small frame: eight handles do not fit] → priority to the handle nearest the pointer, a hit zone of 14 px,
  the body of the frame does not intercept a handle gesture (t763).
- [Changing the preview fraction changes the cache keys and the decode step] → this is already so at every change
  of the fraction; the new pair is computed in the same place as the former one.
- [A curve under the gesture: the person does not expect the drag to set a key] → the rule is the same as for the field in the
  FX window, and the owner has already accepted it in R.60; an automated parameter has no other place for a value.

## Lessons from execution (R.64)

- **The scale "screen → canvas" is computed by the PICTURE, not by the preview panel.** The panel has letterbox
  margins, and translating 48 px of the pointer through its width gave a different number of fractions. Test
  t760 was red on the CORRECT code while it took `preview_rect`; the rectangle of the picture is now
  exposed outward by the field `KtApp::preview_img_rect` — the live scenario and all the harness gesture tests use it too.
- **A gesture is keyed by the POINT OF THE PRESS (`press_origin`), not by the fact that "a drag is going".** A handle
  can be chosen only by the press point (by the first frame that reaches the control, the pointer has
  already left the hit zone), and the geometry snapshot is taken once per press: egui reports
  stop/start in the middle of a movement, and re-taking counted the distance covered twice (60.5 px instead of 48).
- **`drag_at` in the harness switched off the frame modifier**: it sent `PointerButton { modifiers: NONE }`,
  and Shift did not reach the 15° step. The modifier now travels BOTH as a separate event AND in the button itself —
  a relative of the lesson `ctrl_click_at` (R.57).
- **A gesture available ONLY with Alt does not reach the program.** Alt+drag is intercepted by the window manager (openbox, GNOME, KDE — moving a window). Caught by the live scenario `geomctl`;
  in the harness this is not visible at all. The anchor now is grabbed at its point by a narrow zone (8 px),
  and Alt only widens it to the common one.
- **A point probe of a pixel does not prove that the picture moved.** On colour bars a move
  of 60 px leaves the point inside the same bar. The live scenario compares the FINGERPRINT of the preview area
  (a 14×8 grid): a real move gives a difference of 78 levels, the stirring of the thin outline of the frame when the anchor moves — 0.2.
- **The `synth:` source of the picture does not give** (a flat green fill, the sound is real): the scenarios
  about geometry need a file with structure — `pattern_media` (SMPTE bars) next to `color_media`.

## Migration Plan

The project format does not change; the state "show the controls" is the viewport, old files are read as they are. Rollback — switch the controls off: the parsing of a click on the preview returns to today's.

## Found on the owner's project (2026-09-25, `sources/paintitblack.otio`)

The project was opened with the built binary in a virtual screen (the owner's media is NOT on this machine —
the paths lead outside the tree, the sources get stubs, item 209). The frame appears at
the selected event and lies exactly on the picture. But the project rule is "as is", and the stub is small, and the frame came out 45×25 px: **eight handles with a hit zone of 14 px cover it
entirely, and there is nothing to grab the BODY with**. The priority of the nearest handle (t763) saves from a miss between
handles, but does not leave room for the move.

Deliberately deferred (it is not in the tasks of this round): when the frame is smaller than about 3 hit zones of the scale handle,
the handles must be moved OUTSIDE its edges (as Resolve and AE do), and the body is left inside.
This is a change to the SCREEN geometry of the handles (the pure functions of `kt-fx` have no way to know it) plus
its own test — separate work, not a tail of this one.

## Deferred

- **The form of the fields in ALL other windows and widgets** — the owner's direct words of R.64: "and do not forget
  to go over this in all other widgets and windows too in the future". Here only the geometry is brought in line; the rest is separate work with its own walk of the windows (the `WINDOWS` registry of R.63 gives
  a ready list of places).
- The screen controls of CAMERAS and `Pan3d` (world px, the active camera under the cursor).
- Guides (safe zones, grid, center) and "before/after" comparison modes — separate forks.
