# Spec Delta

## ADDED Requirements

### Requirement: Transform frame over the preview picture — gestures, damping, one arithmetic with render
The preview SHALL show the transformation frame of the leading node "Geometry" of the SELECTED video event while the display of the controls is on: the outline of the frame, eight scale handles, a rotation handle, the anchor point, and four crop handles. A gesture over the frame SHALL edit the `EventGeometry` of the same event: the body — the position, the corner and side handles — the zoom, the rotation handle — the angle, the anchor point — the anchor, the crop handles — the crop. All gestures SHALL run at the tempo of the gesture (the picture is rebuilt on each movement) and be recorded in the history in ONE step per gesture.

The geometry of the frame on screen SHALL be computed by THE SAME placement function that builds the geometry program of the render, and the inverse recomputation "shift on the canvas → parameter value" SHALL be its pair: a second placement arithmetic is not introduced into the program. The number that the gesture gives and the number in the field of the FX window SHALL coincide.

Dragging the ANCHOR MUST NOT move the picture: the position is compensated in the same history step, and this is true at any rotation of the layer. The anchor SHALL be grabbed at ITS OWN point without a modifier (a narrow zone, so as not to take the move of the picture — it stands in the centre of the frame), and the modifier SHALL only widen its zone to the common one: a gesture available ONLY with Alt does not reach the program at all — Alt+drag is intercepted by the window manager. The other handles SHALL be sufficient to hit at the first try (a hit zone of at least 14 px even with a frame a few pixels in size) and MUST NOT overlap each other: when the frame is smaller than the sum of the hit zones, the priority goes to the handle nearest the pointer.

For a parameter with a LIVE CURVE the gesture SHALL edit the KEY under the cursor — by the same rule as the field of the FX window — and not a constant; the frame SHALL show the EFFECTIVE geometry (the base under the curves) in that case, that is, exactly what the render draws.

The controls SHALL hide during playback and return on pause; the person SHALL have a toggle for display, and its state is a property of the workspace (the viewport), not of the project content. Without a gesture the controls MUST NOT raise the cost of a frame: neither allocations in the draw path, nor repaint at rest (invariant 12). An event without a leading node (audio, a foreign placeholder) does not get a frame, and the impossibility is named by the status line, not silence.
Why: the owner's R.63 item 6 — "AT MINIMUM we should have editing right in the preview that is convenient"; the industry hides the controls during playback (FCP — always, AE — automatically), and the compensation of the anchor on a rotated layer is a defect of After Effects confirmed by quotes, which we have no reason to repeat (`docs/features/ui/viewer/on-screen-controls.md`).

#### Scenario: Move, zoom and rotation edit the same numbers as the fields
- **WHEN** a video event is selected; the frame is dragged by the body, by the corner handle and by the rotation handle
- **THEN** the `pos_x`/`pos_y`, `zoom` and `rot` of the event change; the values in the fields of the FX window match the screen gesture to the last digit; one history step per gesture; the intermediate frames go to the player; rotation with Shift goes in 15° steps
- **TEST** t760, t760b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The anchor moves, the picture stays — and on a rotated layer too
- **WHEN** the anchor is dragged on a layer without rotation and on a layer rotated by 30°
- **THEN** the anchor values change, while the placement of the picture on the canvas remains the same in both cases (comparison of the placement rectangle before and after); the position compensation is part of the same history step
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`), t761b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Crop with its own handles, the margins are transparent
- **WHEN** the four crop handles are dragged
- **THEN** `crop_l`/`crop_t`/`crop_r`/`crop_b` change; the frame follows the cropped view by the conform rule; the cropped-off part is transparent (not black)
- **TEST** t762 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Hitting a handle and priority with a small frame
- **WHEN** the pointer is moved onto each of the handles, including on a frame smaller than the sum of the hit zones; separately — onto the anchor point and onto a point a few pixels from it
- **THEN** the hit zone of each handle is at least 14 px, the hit is unambiguous (the nearest to the pointer), the body of the frame does not intercept a handle gesture; the anchor is grabbed at its own point, aside from it — already the body; with a modifier — the zone is common
- **TEST** t763 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A gesture over an animated parameter sets a key
- **WHEN** a curve lives on `pos_x` of the event, and `pos_y` has none; the frame is dragged by the body
- **THEN** the constant `pos_x` is intact, a key appeared in the curve at the cursor with the new value; `pos_y` edits the base, a curve is not created for it; one history step; before the gesture the frame showed the value of the CURVE under the cursor
- **TEST** t762b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Controls fade during playback and do not heat the idle state
- **WHEN** playback is started; then pause; then idle with no movement of the pointer
- **THEN** during playback there are no controls, on pause they have returned; at rest there are no additional repaints and no allocations in the draw path
- **TEST** t764 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live scenario of a gesture over the picture
- **WHEN** in the real kt-app an event is selected, the frame is dragged by the body, by the corner, by the rotation and by the anchor
- **THEN** a pixel of the preview changes on the move, the scale and the rotation, and does NOT change on the move of the anchor; the file after saving carries the new geometry values
- **TEST** `src/scripts/live/geomctl.py` (live scenario, `./scripts/live-check.sh`)

### Requirement: The preview share is similar to the project frame
The fraction of the preview SHALL give a size similar to the project frame: the aspect ratio of the preview differs from the aspect ratio of the project by no more than half a pixel along the short side. The conform rule on the preview and on the export SHALL give one and the same frame in fractions of the canvas.
Why: even rounding of both axes independently gave 1920×1080 → 240×134 at 1/8 and 120×66 at 1/16 (1.791 and 1.818 against 1.778), that is, "fit" computed a different frame on the preview than in the export, and the on-screen control would show something other than what goes into the file.

#### Scenario: All fractions are similar to the project frame
- **WHEN** the preview fraction is 1/1, 1/2, 1/4, 1/8 and 1/16 with a project frame of 1920×1080 and of 1080×1920
- **THEN** the aspect ratio of each fraction matches the project's within half a pixel; the "fit" frame in fractions of the canvas is the same on all fractions
- **TEST** t765 (`src/apps/kt-app/src/app.rs`, `src/libs/kt-fx/tests/conform.rs`)

## MODIFIED Requirements

### Requirement: Leading node in the chain strip and thumbnails in source proportions
The chain strip of a video event SHALL show the leading node "Geometry" first on creation, with the leading flag; the cross on the leading node SHALL give an error with a reason, not be dimmed; the catalog SHALL contain "Geometry" as an addable node; the conform rule SHALL be localized in all five catalogs. The captions of the fields of the leading node SHALL carry the same publication door as the captions of any slot (address `ParamOwner::Geometry`): inside a reel a right click on the caption of zoom, position, crop, rotation or anchor of the leading "Geometry" publishes the handle, as for the second "Geometry" node in the chain. The thumbnails of the filmstrip SHALL be ordered as the kind of source in the frame of the thumbnail and be drawn with margins of the row's colour; scrolling and zooming still do not generate decode requests.
The form of the leading node in the FX window SHALL be a "Transformation" section modelled on the industry: collapsible groups (frame / position / crop), X/Y pairs on one row, a unit for each number (× for zoom, ° for rotation, % for crop and anchor), a slider beside the number at zoom and rotation, a frame scheme showing the crop and the anchor, and one action "reset geometry". The value shown by the field and the value given by the screen gesture SHALL be ONE number (rule 17).
Why: the combo `Fit/Fill/Stretch` showed `Debug` names and did nothing; the filmstrip of a vertical clip was squashed — the first place where the user could see the defect. R.63: "why does the first geometry have no way to publish its parameters, while the second has one?" — the leading node's address is the field `geometry` of the event, and there was no owner's variant for it; the door was closed deliberately (`(!lead_sel).then(...)`). R.64: "solid boring input fields, even Vegas does it better" — and the owner's direct clarification "item 2", that is, a "Transformation" section with groups, sliders and a scheme, not a bare grid of numbers.

#### Scenario: The leading node in the editor
- **WHEN** the chain editor of a new video event is open; the cross on the leading node is pressed; the node is dragged after Blur; a second "Geometry" is added
- **THEN** the leading node is first and marked; the cross — an error with a reason; the reordering and the second node — one history step each; the rule captions come from the catalog, the glyphs are in the font
- **TEST** t716b (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: Changing the rule on pause rebuilds the preview
- **WHEN** the project is on pause, and in the settings window the rule is changed from "fit" to "fill"
- **THEN** the preview is rebuilt without any movement of the cursor: a pixel of the field becomes a pixel of the source; the project version is part of the key of the preview
- **TEST** t722b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/conform.py`

#### Scenario: Thumbnails of a vertical clip
- **WHEN** a vertical clip is on a track with a filmstrip; the timeline is scrolled and zoomed
- **THEN** the thumbnail is in the proportions of the source with margins on the sides (centred in the cell); the sidecar table `thumb_meta` carries the size of the media thumbnails (old squashed ones are not read); there are no decode orders on scrolling/zooming
- **TEST** t719 (`src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The leading geometry is published by the same door
- **WHEN** inside a reel a right click on the "Zoom" caption of the leading node and on the "Zoom" caption of the second "Geometry" node; outside a reel — the same
- **THEN** in both cases inside the reel there is a publish item (in the former code the leading node does not have it), the handle lands in the reel with the address `Geometry`; outside the reel there is no item; the instance field shows and edits the zoom of the leading geometry, the preview of the instance changes
- **TEST** t752b (`src/apps/kt-app/src/reach_tests.rs`), t752c (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The "Transform" form instead of a bare grid of numbers
- **WHEN** the leading "Geometry" node of a video event is open; the groups are collapsed; zoom and rotation are edited by a slider and by a number; "reset geometry" is pressed
- **THEN** the position and anchor stand as X/Y pairs on one row, each number has its own unit, the frame scheme shows the crop and the anchor, the folding of a group is the state of the window (not of the project), the slider and the number edit ONE value, the reset returns the node to the default in one history step
- **TEST** t759 (`src/apps/kt-app/src/layout_tests.rs`), t759b (`src/apps/kt-app/src/i18n_tests.rs`)
