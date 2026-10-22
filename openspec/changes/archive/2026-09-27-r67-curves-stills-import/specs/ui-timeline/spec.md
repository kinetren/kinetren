# Spec Delta

## MODIFIED Requirements

### Requirement: Shapes, derived graph and formula in the automation widget
The shape of a segment SHALL be edited from the same automation widget as the keys: by right-clicking
the SEGMENT between two keys (neighboring segments are not touched), with the shape's parameters
shown next to it; the same actions are available on a selection of several segments. The target of a right click (a segment
or a key) SHALL be FIXED AT THE MOMENT of pressing and kept while the menu is open: moving the pointer
to the menu items MUST NOT close it and MUST NOT change the target; the segment under the menu
SHALL be highlighted on the graph, and the menu's header SHALL name its boundaries (the times of both
keys of the material and the fractions of the event) — the person sees BETWEEN WHAT they set the shape. The graph SHALL
draw the VALUE of the curve — a line built by the same evaluator that the render uses, with a step
no coarser than one frame per pixel — and not the segments between keys: a zigzag is seen as a zigzag,
a step as a step, a segment formula as its own line; there is no second drawing formula.
A segment with a shape that has a parameter SHALL have a HANDLE under the mouse at the middle of the segment:
a vertical drag edits the main parameter of the shape (tension for the third order and ease,
amplitude for the zigzag), a horizontal drag with Shift — the number of periods; the handle is drawn on the
segment under the pointer and on the selected one, the number is shown next to it for the duration of the drag, the history step is
one per gesture.
The block "curve as parameter" (choice of the shape of all segments and its parameters, the formula layer, link,
delete the curve) SHALL be ONE per effect slot, generator and reel instance, and SHALL
refer to the curve that the person SEES as selected: the block's header names it by its name
and color, and the curve is selected by clicking its name, its line or its field; a block
without a named curve MUST NOT exist.
A curve with a DERIVED source (link or formula) SHALL be drawn distinguishably from a keyed
one and MUST NOT be edited with the mouse: it has no keys, and dragging a point would silently
create a third truth about the value; at the same time its VALUES SHALL be drawn — resolved by the same code as the render (`resolve_derived`), and not from the empty list of
keys; the `t` of the preview SHALL be counted from the start of the material, as in the render. A curve with a
LAYER-formula SHALL be drawn with two lines: `k` (keys with shapes, edited with the mouse) and the
layer's result (dashed, not editable) — the person sees both what they set and what
sounds. A derived curve SHALL show what drives it — the source address or the text of the
formula — and have one action "unlink", which turns it into ordinary keys by its current
values. A link SHALL be assigned through the "Driven…" DOOR in the same block: the choice of the event (by
default its own) and of a property from the passport of its carriers (slots, geometry, generator), with the
same recalculation text next to it; an unresolvable address is named as an error, not as silence.
The formula field SHALL hold TYPED text between frames (the text lives in the state of the
widget, and is not re-read from the model each frame): an unfinished input is not erased,
a parse error is shown with its position and name and is not applied to the model; the text
is applied to the model on confirmation (Enter, loss of focus) — and MUST NOT be applied on the
first legal prefix (typing `t*2` does not turn the curve into "t" after the first letter, the keys
are not zeroed before confirmation). The formula reference SHALL be IN THE BLOCK (a collapsible
group: the variables `t`/`k`/`src`/`s`/`a`/`b` with their purpose, functions with arity, operators)
and in the status line (the field's hint names the variables; the "no name" error lists the
nearest by prefix); Tab in the field SHALL complete the name by prefix in place, without a list
over the interface. The default shape and the command "apply to all" are reachable from the same
place as the other actions on the curve. The width of the numeric fields, the formula field and the choice of
shape SHALL be fixed by the width of the longest value and MUST NOT depend on the
window width. All new controls SHALL have a hint in the status line.
Why: the owner asked for "the resulting graph is also drawn, but not edited with the mouse";
an editable derived graph would be exactly the "mute dead end" for which the project paid for seven rounds. R.63: "I type it in, and it either erases the formula or nothing
happens" — the field re-read the model each frame, applied on the first legal letter
and zeroed the keys, and the graph drew empty `keys`; "the context menu simply disappears,
I don't have time to reach it" — the target was computed from `hover_pos()` each frame. R.67: "visually
the user cannot see at all what kind of curve it is, and there is not even a way to do anything with the mouse
instead of typing digits" — the graph drew straight lines between keys, and t698 checked the
model, not the drawing; "only one field changes, although I set zigzags in two fields" —
"all segments shape" silently affected the curve that was active; "view curves — I don't
understand at all how to assign them" — there was no door; "no reference, no hints at all";
"the fields grow in width with the window again". Pop-ups over the interface are forbidden (`ui-reach`, rejected),
so the reference is a group in the block, and the completion is Tab in place.

#### Scenario: The shape is edited per section, neighbours intact
- **WHEN** a right click on a segment selects a shape, then its parameter is edited, then the shape is applied to all segments of the curve
- **THEN** only the selected segment and only its parameters change; the graph is redrawn by the same value function that the render computes; "to all" — one undo step
- **TEST** t698 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A derived curve is visible but not edited with the mouse
- **WHEN** the handle has a link source; a drag and a double click are made on its line; then "unlink" is pressed
- **THEN** the line is drawn distinguishably and labeled with its source; neither the drag nor the double click changed the model; "unlink" gave ordinary keys by the current values in one step
- **TEST** t706 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula field explains the error and does not spoil the model
- **WHEN** text with an unclosed bracket, then with an unknown name, then a correct one is typed into the formula field
- **THEN** the first two show an error with the position in the line and do not touch the model; the correct one is applied; the field has a hint in the status line
- **TEST** t709 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula is typed in full, the keys stay alive until confirmation, the curve is drawn
- **WHEN** on a curve with three keys `sin(t)*0.5` is typed into the formula field character by character with pauses of several frames, then Enter; then `t*` is typed and the focus is moved away; then the text is erased
- **THEN** after each character the field shows what was typed (on the previous code `sin(` disappeared on the next frame); until Enter the curve's keys are intact and the model is untouched (on the previous code `t` was applied on the first letter and zeroed the keys); after Enter the source is the formula, and the graph draws a line with a non-zero number of points, equal to `resolve_derived` at the same ticks; the unfinished `t*` is not applied when focus leaves, the error is named; empty text returns the keys by "unlinking", not silently
- **TEST** t749 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

#### Scenario: The section menu stays under the pointer and says what the shape is between
- **WHEN** a right click on a segment between keys 1.0 s and 2.0 s; the pointer is moved 40 px into the menu and the shape is clicked; a right click on a key and the same movement toward the item "Delete"
- **THEN** the menu did not close from the movement (on the previous code it disappeared on the very first frame outside the segment), the header names 1.000 and 2.000 s and the fractions of the event, the segment is highlighted (a backing is drawn between the x of the keys), the shape was applied to this segment; the key's menu is held the same way and deletes exactly that key
- **TEST** t751 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

#### Scenario: The graph draws the shape as it sounds
- **WHEN** a curve has a zigzag segment (4 periods, amplitude 0.2), a step segment and a formula segment `a + (b-a)*s*s`; the graph is drawn at two zoom levels
- **THEN** the drawn line at 16 equally spaced x between the keys of each segment matches `curve_value` at the same ticks to within a pixel (on the previous code — a straight line between keys: a deviation of ≥ 10 px at the zigzag peak); the zigzag has no fewer than 8 extrema; the number of points of the line is not less than the width of the segment in pixels / 2
- **PATH** open the FX window of an event with an effect → add a parameter curve → place three keys by clicking on the graph → right click on a segment → "Zigzag" → between the keys a trembling line is visible on the graph, not a straight one
- **TEST** t784b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The shape parameter handle under the mouse
- **WHEN** the pointer hovers over the middle of a zigzag segment, the handle is dragged up by 20 px; on a third-order segment — down; on a zigzag segment with Shift — to the right
- **THEN** the zigzag amplitude grew, the tension decreased, the number of periods grew — each in one history step; during the drag the number is shown next to the handle; the graph is redrawn at the new value; a straight line and a step have no handle
- **PATH** hover over the middle of the zigzag → a small square appears → drag it up → the zigzag became taller, the amplitude number is shown next to it → release → Ctrl+Z restores it
- **TEST** t785b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The curve block refers to the visible curve and is one for all carriers
- **WHEN** an effect slot has two curves; the person clicks on the name/line/field of the second, chooses "all segments shape — zigzag" and edits the amplitude; the same in the generator and in the head of the reel instance
- **THEN** the zigzag landed on the SECOND curve, the first is untouched (on the previous code the shape landed on the first curve, which was invisibly active); the block's header names the second curve by its color; the reel instance's block is the same (the same fields and the same labels, one code) — "all segments shape", the layer, the link and the deletion are available there too
- **PATH** FX window → two curves → click on the name of the second → the block under the graph is labeled with its name and color → choose "Zigzag" → the second line trembles, the first as it was
- **TEST** t785, t791 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The formula layer over the keys, in the graph and in the field
- **WHEN** on a curve with keys and a zigzag, `k * 0.5 + 0.1` is typed into the layer field and Enter is pressed; then the field is cleared and Enter is pressed
- **THEN** on the graph there are two lines: `k` (solid, keys with diamonds) and the result (dashed), the keys and the zigzag are intact; after clearing there is no dashed line, `k` is as before; the edits go into the FX window session (one history step per session, as for the whole window); when the field is closed, focus is returned (space again sets pause)
- **PATH** curve block → "Layer" field → type `k*0.5+0.1` → Enter → a dashed line half as low appeared above the keys → erase → Enter → the dashed line disappeared
- **TEST** t787e (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The link door assigns the source
- **WHEN** in the curve block "Driven…" is pressed, the person's own event and a property of the neighboring slot are chosen; then another event and its property; then the link is removed with "unlink"
- **THEN** the curve has a link source with the address of the chosen property, the curve's name shows the address, the line is dashed; for the other event the address carries its number; an impossible address (across the reel boundary) is named as an error in the status line; "unlink" returns the keys; the item is reachable from the FX window, the generator and the reel instance
- **PATH** curve block → "Driven…" → list "this event / others" → choose a property → the curve's name became "amount <- fx.0.radius", the line is dashed → "Unlink" → the keys are back
- **TEST** t788 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: Formula reference in the block and in the line, completion by Tab
- **WHEN** the group "Reference" in the curve block is opened; `cl` is typed into the formula field and Tab is pressed; `foo(t)` is typed and Enter is pressed; the cursor stays in the field
- **THEN** the group lists all variables and all evaluator functions with arity (the list equals the evaluator's registry, not a copy); Tab completed to `clamp(` without a pop-up list; the error names `foo` and the nearest names; the status-line hint at the field names `t`, `k`, `src`; no layer above `Order::Middle` appeared
- **PATH** curve block → "Reference" → you see "t — seconds of the material", "k — keys…", `clamp(x, lo, hi)`… → in the field type `cl`, Tab → it became `clamp(` → type `foo(t)`, Enter → at the bottom an error "no name foo; there is floor"
- **TEST** t789 (`src/apps/kt-app/src/layout_tests.rs`), t789b (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: Field width does not depend on the window width
- **WHEN** the FX window with a curve block is opened at screen widths of 900 and 1800 px; the rectangles of the numeric fields, the formula field, the layer field and the shape choice are measured
- **THEN** the width of each field is the same at both widths (on the previous code the shape choice and the formula field grew past the window); the width of a numeric field equals the width of the longest value of the passport plus the field
- **PATH** open the FX window → stretch the main window twice → the fields in the curve block stayed the same width
- **TEST** t790 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Transform frame over the preview picture — gestures, damping, one arithmetic with render
The preview SHALL show the transform frame of the leading "Geometry" node of the SELECTED
video event, while the display of controls is on: the frame outline, eight scale handles, the
rotation handle, the anchor point and four crop handles. A gesture over the frame SHALL edit the `EventGeometry`
of that same event: the body — the position, the corner and side handles — the zoom, the rotation handle — the angle,
the anchor point — the anchor, the crop handles — the crop. All gestures SHALL run at the tempo of the gesture (the picture
is rebuilt on each movement) and be recorded in the history in ONE step per gesture.

The frame geometry on the screen SHALL be computed by THE SAME placement function that builds the render's
geo program, and the inverse recalculation "shift on the canvas → parameter value" SHALL be its pair:
a second placement arithmetic is not introduced into the program. The number that a gesture gives and the number in
the FX window field SHALL coincide.

A SMALL FRAME has no right to eat the move: when a side of the frame on the screen is less than three
hit zones, the eight scale handles SHALL be drawn and caught OUTSIDE the edges of the frame (outside), and
its body SHALL remain under the move. The rotation handle and the anchor point remain in their
places. The rule SHALL act from the size ON THE SCREEN, not from fractions of the canvas: the same
geometry at different fractions of the preview gives a different frame.

Dragging the ANCHOR MUST NOT move the picture: the position is compensated by the same history step,
and this holds for any rotation of the layer. The anchor SHALL be grabbed at ITS OWN point without a modifier
(a narrow zone, so as not to take the picture's move — it sits in the center of the frame), and the modifier
SHALL only widen its zone to the common one: a gesture available ONLY with Alt does not reach the program
at all — Alt+drag is intercepted by the window manager. The other handles SHALL be
sufficient to hit on the first try (a hit zone of at least 14 px even with a frame of a few pixels) and MUST NOT
overlap each other: when the frame is smaller than the sum of the hit zones, the priority goes to the handle nearest the pointer.

A parameter with a LIVE CURVE, the gesture SHALL edit the KEY at the cursor — by the same rule as the
FX window field — and not a constant; the frame, meanwhile, SHALL show the EFFECTIVE geometry (the base under the
curves), that is, exactly what the render draws.

The controls SHALL fade during playback and return on pause; the display of the controls SHALL be
ON by default (the "Transform" mode), and the switch — on the preview itself
(a button in the corner of the frame) and in its menu; the state of the switch is a property of the workplace,
lives in the user storage and comes back at the next launch: turned off — it is remembered, reinstalled the program — it is on again. The frame
scheme in the "Transform" form SHALL FIT the frame into its rectangle at any zoom and position
(the scale of the scheme is chosen by the span of the frame and the picture, the picture is drawn inside the same scheme): at 2× zoom
a frame twice as large as the picture is visible, not an empty rectangle. Without a gesture the controls MUST NOT raise the cost of a
frame: neither allocations in the draw path nor a repaint while idle (invariant 12). An event without a leading
node (audio, a foreign placeholder) gets no frame, and the impossibility is named by a status-line
string, not silence.
Why: R.63 item 6 of the owner — "AT LEAST we must have convenient editing right in the preview"; the industry hides the controls during playback (FCP — always, AE — automatically), and the anchor compensation on a rotated layer is a defect of After Effects confirmed by quotes, which we have no reason to repeat (`docs/features/ui/viewer/on-screen-controls.md`). R.67: "instead of control in the preview we have only meager control inside the fx widget" — the frame was off by default, was not remembered and was switched on only from the right click on the preview, that is, for the person it did not exist (checklist rule 4: a useful thing is on by default); "if you scale even 2x - you simply have nothing in the preview" — the scheme was clipped by its own rectangle.

#### Scenario: Move, zoom and rotation edit the same numbers as the fields
- **WHEN** a video event is selected; the frame is dragged by the body, by the corner handle and by the rotation handle
- **THEN** the event's `pos_x`/`pos_y`, `zoom` and `rot` change; the values in the FX window fields match the on-screen gesture to the last digit; each gesture is one history step; the intermediate frames go to the player; rotation with Shift goes in 15° steps
- **TEST** t760, t760b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The anchor moves, the picture stays — and on a rotated layer too
- **WHEN** the anchor is dragged on a layer without rotation and on a layer rotated by 30°
- **THEN** the anchor values change, while the placement of the picture on the canvas stays the same in both cases (a comparison of the placement rectangle before and after); the position compensation is part of the same history step
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`), t761b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Crop with its own handles, the margins are transparent
- **WHEN** the four crop handles are dragged
- **THEN** `crop_l`/`crop_t`/`crop_r`/`crop_b` change, the frame follows the cropped view by the frame-conform rule, and the cropped-away part is transparent (not black)
- **TEST** t762 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A small frame gives its handles outward, the body stays its own
- **WHEN** the frame is less than three hit zones on a side (the "as is" rule with a small source or a placeholder): the pointer hovers over each scale handle and over the center of the frame
- **THEN** the handles are caught OUTSIDE the edges of the frame and drawn there too, the center of the frame gives a MOVE, not a handle; on an ordinary frame the handles remain on the edges, as before
- **TEST** t781 (`src/apps/kt-app/src/layout_tests.rs`), t782 (`src/libs/kt-fx/tests/placement.rs`)

#### Scenario: Hitting a handle and priority with a small frame
- **WHEN** the pointer hovers over each of the handles, including on a frame smaller than the sum of the hit zones; separately — over the anchor point and over a point a few pixels away from it
- **THEN** the hit zone of each handle is at least 14 px, the hit is unambiguous (the nearest to the pointer), the frame's body does not intercept the handle's gesture; the anchor is grabbed at its own point, aside from it — already the body; with the modifier — the zone is common
- **TEST** t763 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A gesture over an animated parameter sets a key
- **WHEN** `pos_x` of the event has a live curve, and `pos_y` has none; the frame is dragged by the body
- **THEN** the constant `pos_x` is intact, a key appeared in the curve at the cursor with the new value, `pos_y` edits the base, no curve is created for it, the history step is one; before the gesture the frame showed the value of the CURVE under the cursor
- **TEST** t762b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Controls fade during playback and do not heat the idle state
- **WHEN** playback is started; then pause; then idle without pointer movement
- **THEN** during playback there are no controls, on pause they are back; in idle there are no extra repaints and no allocations in the draw path
- **TEST** t764 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame is there at once, switching off is remembered, the diagram fits
- **WHEN** the program starts without storage; a video event under the cursor is selected; then the display is switched off with the button on the preview and the program is restarted; separately — on the event a zoom of 2× and a position past the edge of the frame are set, the "Transform" form is opened
- **THEN** on the first launch the frame is drawn without a single action (on the previous code — never, until the right-click item is found); after switching off and restarting there is no frame, and the button shows "off"; deleting the storage returns the default "on"; the form's scheme draws both the picture and the frame entirely within its rectangle (all four corners of the frame are inside; on the previous code — none of them)
- **PATH** open the project → click the video event → at once the frame with handles on the picture → the button in the corner of the preview switches it off → close and open the program → no frame, the button "off" → switch it on → it is there
- **TEST** t794 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/geomctl.py`

#### Scenario: Live scenario of a gesture over the picture
- **WHEN** in the real kt-app the event is selected, the frame is dragged by the body, by a corner, by the rotation and by the anchor
- **THEN** the preview pixel changes on the move, the scale and the rotation and does NOT change on the anchor move; the file after saving carries the new geometry values
- **TEST** `src/scripts/live/geomctl.py` (live scenario, `./scripts/live-check.sh`)

## ADDED Requirements

### Requirement: Head of the chain strip of a reel instance — a short label, not an accent
The head of the chain strip at a reel instance SHALL be labeled with a pin and the NAME OF THE REEL,
truncated by the same rule as the slot name (the full name is in the status-line hint), and
drawn in the color of an ordinary slot, not in the accent; the selected head SHALL occupy the same
width as the unselected one. The chain strip with the head SHALL leave the slots no less
room than without it, minus one slot width.
Why: R.67, the owner: "why does the user need to cover fx-chain with a long inscription
"Composition parameters"? It's also blue... if you select it, you get a long blue rectangle" — checklist rules 7 (the text fits) and 13 (do not waste space).

#### Scenario: The head is short and not blue
- **WHEN** a reel instance with a name of 40 characters is opened in the FX window; the head is selected
- **THEN** the width of the head is no more than the width of a slot with the longest name (truncated with an ellipsis), the color of the label equals the color of a slot label, the width of the selected head equals the unselected one; the status-line hint carries the full name
- **PATH** FX window of the instance → at the start of the strip "📌 Reel…" the width of a slot → click on it → the highlight of the same width → the full name at the bottom of the line
- **TEST** t793 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/forms.py`
