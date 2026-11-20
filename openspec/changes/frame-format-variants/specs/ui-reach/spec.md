## ADDED Requirements

### Requirement: Formats are switched in the preview and framed by gesture
The preview's head SHALL offer the master and each format; with a format shown, the on-screen geometry
frame and the Geometry node's fields SHALL write that format's override of the event, created on the
first gesture only; the event SHALL show a mark when it has an override for the shown format; its
right-click menu SHALL offer "Reset framing to master" and "Hide in this format"; formats SHALL be
created in the project settings window's Formats section with presets; a safe-area overlay of the
format's preset SHALL be drawn by the preview only.
Why: UI rules 13, 17, 20, 22.

#### Scenario: A vertical by mouse
- **WHEN** "+" in the preview head adds a 9:16 format from presets, the vertical is shown, the speaker's event is dragged in the frame, the safe-area toggle is pressed, the event's menu → "Reset framing to master"
- **THEN** the preview shows the vertical; the drag moves the event only in the vertical (the master unchanged when switched back) and the event shows the override mark; the overlay shows the platform zones; the reset removes the override in one undo step
- **PATH** click "+" in the preview head → the settings window's Formats section → preset "9:16" → close → click "9:16" → the vertical frame → drag the speaker in the frame → click "16:9" → the master is unchanged → click "9:16" → the moved framing → click the safe-area toggle → zones drawn → right click on the event → "Reset framing to master" → the default framing
- **TEST** t3491 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/formats.py` (t3492)
