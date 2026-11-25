## ADDED Requirements

### Requirement: The Scopes window is reached from View and from the preview
The Scopes window SHALL be a row of the window registry with its reason, opened from View and from the preview's
right-click menu by one body, and built by the window shell as a tool window. Its instruments and scale SHALL be
remembered per person, not in the project. False colour SHALL be a toggle of its head, marked on the picture while on,
and Esc SHALL turn it off.
Why: UI rules 17, 22, 23; kkt-26.11.20 A8.

#### Scenario: Opening and arranging the scopes by mouse
- **WHEN** View › Scopes is clicked, "Parade" is pressed, the scale is set to %, False colour is pressed, Esc, the window is closed and opened again from the preview's menu, then the program is started again
- **THEN** the window opens with the waveform and the vectorscope of the shown frame, then three instruments with % labels; the picture is painted by zones with its badge and Esc gives it back, the window staying; the choice is the same after the reopening and the restart; the project file holds nothing of it
- **PATH** click "View" → "Scopes" → a window: the waveform and the vectorscope of the current frame, at the right "1280×720" → press "Parade" → three panes → click "%" → labels 0–100 % → press "False colour" → the picture turns into zones with the badge → Esc → the picture is back, the window stays → Esc → the window closes → right click on the preview → "Scopes" → the same three panes
- **TEST** t3411 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `scopes` (t3412)
