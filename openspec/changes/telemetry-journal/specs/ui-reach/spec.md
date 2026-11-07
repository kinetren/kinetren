## ADDED Requirements

### Requirement: Diagnostics shows the journal at any window height, and the moment can be marked
The Diagnostics window SHALL show the telemetry journal's tail, newest at the bottom, each record with
its clock time, in a pane outside the window's scroll that takes the lower part of the body at every
window size down to the window's minimum, so it is readable without enlarging the window. Its head row
SHALL hold "Mark the moment" with an optional note field (Enter marks too) and "Open folder", which
opens the data directory. Ctrl+Alt+M SHALL mark the moment from anywhere in the main window, in every
key style. Notices and the journal show local time.
Why: the owner, kkt-26.11.07b line 17 (G12, found-defects 109): "ОКАЗЫВАЕТСЯ НАДО ОКНО ПО ВЫСОТЕ
УВЛИЧИТЬ И ТОООГДАДАА ОНО ПОЯВИТСЯ"; and he could only guess when a stall happened.

#### Scenario: The journal is on screen at the default and the minimum height
- **WHEN** the Diagnostics window opens at its default size, and again at its minimum size
- **THEN** the journal pane is inside the window's visible body, shows at least six lines, and its mark button is clickable
- **PATH** View → Diagnostics… → the window opens; at its bottom the journal lists the last records with their times, with "Mark the moment", a note field and "Open folder" above them; drag the window's corner up to its minimum → the journal is still there, the graphs above scroll
- **TEST** t1968 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: A mark from the button and from the key
- **WHEN** the person types a note and presses "Mark the moment", then presses Ctrl+Alt+M over the timeline
- **THEN** the journal gets two MARK records in order, the first with the note, and the note field empties
- **PATH** View → Diagnostics… → type "stall here" in the note field → press "Mark the moment" → a line "MARK stall here" appears at the bottom of the journal with the time; close the window, press Ctrl+Alt+M → the status line says the moment is marked
- **TEST** t1969 (`src/apps/kt-app/src/diag_tests.rs`), live `journal` (`src/scripts/live/journal.py`)
