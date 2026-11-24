## ADDED Requirements

### Requirement: A reflection's line and a tile's rectangles are set on the preview
A selected Mirror in reflect mode SHALL show its centre, its angle and its line on the preview, and a selected Tile its
destination and source rectangles, as declared handles; a drag SHALL write the rows (a key where the row is animated) in one
undo step, in the event's, the layer's and the track's window.
Why: the owner's B6, B10 ("нет нормальнго управления эффектом в превью").

#### Scenario: The line and the rectangle by mouse
- **WHEN** a Mirror's centre is dragged and its angle turned, and a Tile's destination frame is dragged by a grip and by its body
- **THEN** the picture follows each drag, the rows change in one undo step per drag, and a rectangle does not leave the canvas
- **PATH** select the event → effects window → the Mirror node → a crosshair and a line on the preview → drag the crosshair to the subject's face: the reflection line goes there → drag the angle handle: the line turns, the picture reflects at the new angle → Ctrl+Z → the former angle → the Tile node → a solid frame over the top half and a dashed one → drag the solid frame's lower grip up: the sliding part gets lower, the rest of the picture appears under it
- **TEST** t3576 (`src/apps/kt-app/src/reach_tests.rs`)
