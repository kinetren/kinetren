## ADDED Requirements

### Requirement: A link reads a tracker's channels
A link SHALL be able to read a tracker point's `x`, `y` and `conf`, and with two points `angle` and
`scale`, by the address of the owning event and tracker; between material frames the value SHALL be
interpolated linearly, and a frame below the tracker's confidence threshold SHALL hold the last good
sample. The apply gestures (attach an event, an effect centre, a corner pin, a mask, stabilize) SHALL
write ordinary links with the expression that maps source pixels to the consumer's space and keeps the
consumer where it stands at the current frame; each gesture SHALL be one undo step.
Why: the owner's T1 "привязывать одно к другому"; F-MSK-005 — one track, many consumers.

#### Scenario: One track drives three consumers
- **WHEN** a tracked point is applied to another event's position, to a Pixelate's centre and to a mask's transform, and the project is played and exported
- **THEN** at every frame the three follow the point (each within 0.5 px of the mapped sample), they stay where they stood at the apply frame, and the links are listed and removable in the curve editor; preview and export are equal
- **TEST** t3523 (`src/libs/kt-engine/tests/tracker_links.rs`)

#### Scenario: Four points pin a screen and two stabilize
- **WHEN** four points tracked on a fixture's moving quadrilateral are applied as Corner Pin, and two points of a shaking fixture are applied as Stabilize
- **THEN** the pinned layer's corners match the quadrilateral within 0.5 px; the stabilized frames' feature stays within 0.5 px of its reference position and rotation within 0.1°
- **TEST** t3524 (`src/libs/kt-engine/tests/tracker_links.rs`)
