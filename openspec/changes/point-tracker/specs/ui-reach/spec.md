## ADDED Requirements

### Requirement: Tracking is done on the preview
A right click on the preview over an event SHALL offer "Track point" and "Track plane"; a selected tracker SHALL show its
boxes and its recent path on the preview and the session controls (Track both ways, backward and forward, one
frame and continuous, stop, threshold, Apply) in the preview's corner row; the event's effects window SHALL list its
trackers in a section; Apply SHALL list the gestures of D5 with what each will link.
Why: UI rule 22 (the place order); masks are edited the same way.

#### Scenario: Track and attach a title by mouse
- **WHEN** the preview is right-clicked on a face → "Track point", forward continuous is pressed, the run stops at a turn, the point is dragged back onto the face and forward pressed again, then Apply → "Attach event…" → the title event
- **THEN** a point with two boxes appears; the path grows as it tracks; the stop is shown in red with its confidence; after the correction the path continues; the title follows the face on playback
- **PATH** right click on the face in the preview → "Track point" → two boxes appear → click ▶▶ in the preview's corner row → the path grows → it stops, the head says "confidence 42 % at 00:00:03:10" in red → drag the point onto the face → ▶▶ → the path continues to the end → click "Apply" → "Attach event…" → click the title event → Space → the title follows the face
- **TEST** t3526 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `tracker` (t3527)

#### Scenario: Track a plane and pin a picture into it
- **WHEN** the preview is right-clicked on a screen in the footage → "Track plane", a rectangle is dragged over the screen, "Track" is pressed, then Apply → "Attach event…" → an image event
- **THEN** a quad with its grid appears; one Track runs both ways from the playhead; the image sits in the screen in perspective on playback
- **PATH** right click on the screen in the preview → "Track plane" → drag a rectangle over it → a quad with a grid appears → click "Track" in the preview's corner row → the quad follows the screen both ways from the playhead → click "Apply" → "Attach event…" → click the image event → Space → the image stays inside the screen, tilted with it
- **TEST** t3531 (`src/apps/kt-app/src/reach_tests.rs`)
