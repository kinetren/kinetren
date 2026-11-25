## ADDED Requirements

### Requirement: Formats are switched on the preview and framed by gesture
With formats in the project, a button at the preview's lower left SHALL offer the master, each format and the safe areas.
With a format shown, the on-screen geometry frame SHALL write that format's correction, created by the first gesture only;
the Geometry node's form SHALL show the correction as a section under the common rows; the event SHALL carry a mark; its
menu SHALL offer resetting the correction and hiding or showing the event in the format. Formats SHALL be made in the
project settings' Formats section from presets. A format's safe areas SHALL be drawn by the preview only.
Why: the owner's ★15; A17, Q27 ("Пресеты платформ"); UI rules 13, 17, 20, 22.

#### Scenario: A vertical by mouse
- **WHEN** a 9:16 format is made from its preset, shown, the speaker's event is dragged in the frame, the safe areas are turned on, the master is shown, its event moved, the vertical shown again, and the event's menu resets the correction
- **THEN** the drag moves the event only in the vertical and marks it; the master's later move shifts it in the vertical too, with the correction over it; the zones are drawn over the preview only; the reset is one undo step
- **PATH** Project settings → Formats → "+ 9:16" → a row "9:16 1080×1920 Fill" → close: a button "16:9" stands at the preview's lower left → click it → "9:16" → the picture is vertical → drag the speaker left in the frame: the event gets a mark on the timeline → the button → "Safe areas" → the platform's zones over the picture → the button → "16:9": the wide picture, the speaker where it was → drag the speaker up → the button → "9:16": the speaker is left and higher → right click on the event → "Reset framing (9:16)" → the speaker at the vertical's centre, higher
- **TEST** t3491 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `formats` (t3492)
