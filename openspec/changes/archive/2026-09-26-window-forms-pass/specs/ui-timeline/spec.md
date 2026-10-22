# Spec Delta

## MODIFIED Requirements

### Requirement: Transform frame over the preview picture — gestures, damping, one arithmetic with render
The preview SHALL show the transform frame of the leading "Geometry" node of the SELECTED video event while the display of controls is on: the outline of the frame, eight scale handles, the rotation handle, the anchor point and four crop handles. A gesture over the frame SHALL edit the `EventGeometry` of that same event: the body — position, the corner and side handles — zoom, the rotation handle — angle, the anchor point — anchor, the crop handles — crop. All gestures SHALL run at gesture pace (the picture is rebuilt on every movement) and be recorded in the history as ONE step per gesture.

The geometry of the frame on screen SHALL be computed by THE SAME placement function that builds the render's geo-program, and the inverse computation "shift on the canvas → parameter value" SHALL be its pair: a second placement arithmetic is not created in the program. The number that a gesture produces and the number in the FX window's field SHALL coincide.

A SMALL FRAME has no right to swallow the carry-over: when a side of the frame on screen is smaller than three grab zones, the eight scale handles SHALL be drawn and caught BEYOND the edges of the frame (outside), while its body SHALL remain under the carry-over. The rotation handle and the anchor point stay in their places. The rule SHALL act on the size ON SCREEN, not on canvas fractions: the same geometry at different fractions of the preview gives a different frame.

Dragging the ANCHOR MUST NOT move the picture: the position is compensated by the same history step, and this holds at any rotation of the layer. The anchor SHALL be grabbed by ITS OWN point without a modifier (a narrow zone, so as not to take the carry-over of the picture — it sits at the centre of the frame), and a modifier SHALL only widen its zone to the common one: a gesture available ONLY with Alt never reaches the program at all — Alt+drag is intercepted by the window manager. The other handles SHALL be sufficient for a hit on the first try (the grab zone is at least 14 px even with a frame a few pixels across) and MUST NOT overlap each other: when the frame is smaller than the sum of the grab zones, priority goes to the handle nearest the pointer.

A parameter with a LIVE CURVE SHALL be edited by a KEY at the cursor — by the same rule as the FX window's field — and not by a constant; the frame SHALL show the EFFECTIVE geometry (the base under the curves) in that case, that is, exactly what the render draws.

The controls SHALL fade during playback and return on pause; the person SHALL have a display toggle, and its state is a property of the workspace (the viewport), not of the project's content. Without a gesture the controls MUST NOT raise the cost of a frame: neither allocations in the draw path nor a repaint while idle (invariant 12). An event without a leading node (audio, someone else's placeholder) does not get a frame, and the impossibility is named by a status line, not by silence.
Why: R.63 item 6 of the owner — "AT LEAST we should have convenient editing right in the preview"; the industry fades the controls during playback (FCP — always, AE — automatically), and compensating the anchor on a rotated layer is a defect of After Effects confirmed by quotations, which we have no reason to repeat (`docs/features/ui/viewer/on-screen-controls.md`).

#### Scenario: Move, zoom and rotation edit the same numbers as the fields
- **WHEN** a video event is selected; the frame is dragged by the body, by the corner handle and by the rotation handle
- **THEN** the `pos_x`/`pos_y`, `zoom` and `rot` of the event change; the values in the FX window's fields coincide with the on-screen gesture to the last digit; each gesture is one history step; intermediate frames go to the player; rotation with Shift goes in steps of 15°
- **TEST** t760, t760b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The anchor moves, the picture stays — and on a rotated layer too
- **WHEN** the anchor is dragged on a layer without rotation and on a layer rotated by 30°
- **THEN** the anchor values change, while the placement of the picture on the canvas stays the same in both cases (a comparison of the placement rectangle before and after); the position compensation is part of the same history step
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`), t761b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Crop with its own handles, the margins are transparent
- **WHEN** the four crop handles are dragged
- **THEN** `crop_l`/`crop_t`/`crop_r`/`crop_b` change, the frame follows the cropped view by the reduction rule, and the cropped-away part is transparent (not black)
- **TEST** t762 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A small frame gives its handles outward, the body stays its own
- **WHEN** the frame is smaller than three grab zones on a side (the "as is" rule with a small source or a placeholder): the pointer is moved onto each scale handle and onto the centre of the frame
- **THEN** the handles are caught BEYOND the edges of the frame and are drawn there too; the centre of the frame gives the CARRY-OVER, not a handle; on an ordinary frame the handles remain on the edges, as before
- **TEST** t781 (`src/apps/kt-app/src/layout_tests.rs`), t782 (`src/libs/kt-fx/tests/placement.rs`)

#### Scenario: Hitting a handle and priority with a small frame
- **WHEN** the pointer is moved onto each of the handles, including on a frame smaller than the sum of the grab zones; separately — onto the anchor point and onto a point a few pixels from it
- **THEN** the grab zone of each handle is at least 14 px, the hit is unambiguous (the nearest to the pointer), the body of the frame does not intercept a handle's gesture; the anchor is grabbed at its own point, and aside from it is already the body — with a modifier the zone is the common one
- **TEST** t763 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A gesture over an animated parameter sets a key
- **WHEN** `pos_x` of the event has a live curve, and `pos_y` does not; the frame is dragged by the body
- **THEN** the constant of `pos_x` is intact, a key appears in the curve at the cursor with the new value, `pos_y` edits the base, a curve is not created for it, the history step is one; before the gesture the frame showed the value of the CURVE under the cursor
- **TEST** t762b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Controls fade during playback and do not heat the idle state
- **WHEN** playback is started; then pause; then idling without pointer movement
- **THEN** during playback there are no controls, on pause they have returned, in idle there are no extra repaints and no allocations in the draw path
- **TEST** t764 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live scenario of a gesture over the picture
- **WHEN** in the real kt-app an event is selected, and the frame is dragged by the body, by the corner, by the rotation and by the anchor
- **THEN** the preview pixel changes on the carry-over, the scale and the rotation, and does NOT change on the anchor carry-over; the file after saving carries the new geometry values
- **TEST** `src/scripts/live/geomctl.py` (live scenario, `./scripts/live-check.sh`)
