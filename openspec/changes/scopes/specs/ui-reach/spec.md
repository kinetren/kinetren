## ADDED Requirements

### Requirement: The Scopes window is reached from View and from the preview
The Scopes window SHALL be a row of the window registry with its reason, opened from the View menu and
from the preview's right-click menu, built by the window shell as a tool window (Esc and the cross
close it, its body fills it), and its choice of instruments and scale SHALL be kept in the project's
UI state, not written at its default.
Why: UI rule 22 — a new window only from the registry, reached where the person acts.

#### Scenario: Opening and arranging the scopes by mouse
- **WHEN** View → "Scopes" is clicked, the vectorscope and parade toggles are pressed, the scale switch is set to %, the window is closed with Esc and opened again from the preview's right-click menu
- **THEN** the window opens with the luma waveform by default, then shows parade and vectorscope side by side with % labels; after reopening the same choice is shown; the project saved and reopened keeps it; a project that never opened the window writes no field
- **PATH** click "View" → "Scopes" → a window with the waveform of the current frame → press "Parade" and "Vectorscope" → three panes side by side → click "%" → labels 0–100 % → Esc → the window closes → right click on the preview → "Scopes" → the same three panes
- **TEST** t3411 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `scopes` (t3412)
