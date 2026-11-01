## ADDED Requirements

### Requirement: Masks are drawn and edited on the preview picture with the frame's gestures
While a mask owner is being edited, the preview SHALL show its masks over the picture through the
same placement the render uses (a picture or slot mask follows the event's Geometry node; a track
mask is in the canvas) and SHALL hide the R.64 frame (one set of handles at a time). Tools: Rectangle
and Ellipse draw by a drag (Shift — square/circle, Alt — from the centre); Pen adds a corner vertex by
a click and a smooth vertex by a click-drag, closes on a click on the first vertex or Enter, Esc drops
an unfinished path, Backspace removes its last vertex. Editing: a drag of a vertex, a tangent, a scale
or rotation handle, the body, or the feather handle on the dashed outer contour; Alt+click on a
vertex toggles corner/smooth; a double-click on a segment inserts a vertex; Delete removes the
selected vertex (a path keeps three — the refusal is shown as an error). Grab zones are the frame's;
the nearest handle wins; every drag is recomputed from the press and is ONE history step; a row with
a live curve gets a key at the cursor only by the gesture. During playback the contours follow the
animation and the handles are hidden. A click on the picture outside every handle and contour still
plays and pauses.
Why: R.64 — "editing right in the preview, convenient"; F-MSK-001's admission that a contour needs a
third kind of editor, introduced once and shared by every owner.

#### Scenario: A rectangle drawn from the preview menu is one step
- **WHEN** a video event is selected under the playhead; the preview's right-click menu → Mask → Rectangle; a drag over the picture; then undo
- **THEN** the event has one rectangle mask covering the dragged box (in source coordinates through the placement), the picture outside it shows the lower layer; the frame's handles are hidden and the mask's are shown; undo removes the mask in one step
- **PATH** right click on the picture → Mask → Rectangle → the cursor is a cross → drag a box over the face → the box stays, outside it the track below shows through, eight handles sit on the box → Ctrl+Z → the mask is gone
- **TEST** t1276 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The pen draws a Bézier path
- **WHEN** the Pen tool is armed; three clicks and one click-drag place four vertices; a click on the first vertex closes; separately — an unfinished path is dropped by Esc
- **THEN** a path mask with four vertices, the dragged one smooth with mirrored tangents, the others corners; after Esc nothing was created and a second Esc leaves mask editing (the frame returns)
- **PATH** Masks → Pen → click, click, click-and-pull → a curved segment appears under the cursor → click the first point → the path closes and cuts the picture → Pen again, two clicks, Esc → the dots disappear → Esc → the transform frame is back
- **TEST** t1277 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: Dragging a point of an animated path keys the whole path
- **WHEN** a path is keyed by its stopwatch at one tick; the playhead moves; a vertex is dragged; separately — a mask without curves is merely drawn for many frames
- **THEN** every shape row has a key at the new tick and only the dragged vertex moved; drawing without a gesture writes nothing; the drag is one history step
- **PATH** in the mask row click the stopwatch → move the playhead → drag a point → the path bends; the graph below shows keys at both ticks on every point
- **TEST** t1278 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The preview click and the playback around masks
- **WHEN** a mask is being edited; a click on the picture far from every handle and contour; then playback starts
- **THEN** the click plays and pauses as before; during playback the contour is drawn at the animated position and no handle is hit-tested
- **PATH** with a mask selected click an empty corner of the picture → playback starts → the mask's line moves with the animation, its points are gone → click again → it stops, the points are back
- **TEST** t1282 (`src/apps/kt-app/src/mask_tests.rs`)
