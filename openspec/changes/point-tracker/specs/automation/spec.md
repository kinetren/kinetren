## ADDED Requirements

### Requirement: A link reads a tracker's channels
A link SHALL be able to read a tracker's channels — a point's place on the canvas and in the source, its confidence, with
two points the turn and the scale, and the geometry that would hold it still — by the address of the owning event and tracker, and SHALL be able to stand on
and read the rows of an event's Geometry node; between material frames the value SHALL be
interpolated linearly, and a frame below the tracker's confidence threshold SHALL hold the last good
sample. The apply gestures (attach an event, an effect centre, a corner pin, a mask, stabilize) SHALL
write ordinary links whose expression only keeps the
consumer where it stands at the current frame (a mask is carried by a reference to the tracker); each gesture SHALL be one undo step.
Why: the owner's T1 "привязывать одно к другому"; F-MSK-005 — one track, many consumers.

#### Scenario: One track drives three consumers
- **WHEN** a tracked point is applied to another event's position, to a Pixelate's centre and to a mask's transform, and the project is played and exported
- **THEN** at every frame the three follow the point (each within 0.5 px of the mapped sample), they stay where they stood at the apply frame, and the links are listed and removable in the curve editor; preview and export are equal
- **TEST** t3523 (`src/libs/kt-engine/tests/tracker_links.rs`)

#### Scenario: Four points pin a screen and two stabilize
- **WHEN** four points tracked on a fixture's moving quadrilateral are applied as Corner Pin, and two points of a shaking fixture are applied as Stabilize
- **THEN** the pinned layer's corners match the quadrilateral within 0.5 px; the stabilized frames' feature stays within 0.5 px of its reference position and rotation within 0.1°
- **TEST** t3524 (`src/libs/kt-engine/tests/tracker_links.rs`)

#### Scenario: A geometry row is linked and read
- **WHEN** an event's position is linked to another event's position with an offset, a scale to a tracker's, and the leader is moved, zoomed, animated and the follower split
- **THEN** every frame shows the follower by the leader's value at that tick; a cycle through geometry rows is refused naming it; the preview, the export and the interface's frame agree
- **TEST** t4255 (`src/libs/kt-model/tests/links.rs`)
