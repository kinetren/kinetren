## ADDED Requirements

### Requirement: Keys and clicks a slow frame reads keep their meaning
Every command key a frame reads SHALL run, once per press, in the order pressed; a command that opens a
window or writes the file runs once a frame, and one that opens a text field ends the frame's keys. Esc
and Enter run once per press. egui's double-click and click windows SHALL widen by the busy frame the
program measures (`KeyClock::busy`, the smoothed gap before a frame the previous one asked for at once);
the harness keeps egui's windows, its clock being a fixed step.
Why: under the full live run a slow frame read → → → ← together and the head did not move (a set of the
frame's commands: +1 −1), two Ctrl+Z undid one edit, and a double click whose two clicks were read a
600 ms frame apart was two single clicks (`formula`, `lessons`, `forms`; found-defects 99, 166). egui
stamps an input with the frame that read it: a gesture's length is known to a busy frame.

#### Scenario: Several keys in one frame
- **WHEN** one frame reads → → → ← with the head on a frame boundary; then one frame reads Ctrl+Z twice
  after two edits
- **THEN** the head is two frames on; both edits are undone
- **PATH** the cursor stands on a frame → the person taps → three times and ← once quickly → the cursor
  is two frames on, whatever the machine's frame time; two quick Ctrl+Z → both edits are gone
- **TEST** t2211 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The click windows widen by the busy frame
- **WHEN** the program has measured frames of 450 ms
- **THEN** egui's double-click window is 0.3 s + 0.45 s and its click window 0.8 s + 0.45 s; in the
  harness both stay egui's
- **PATH** on a loaded machine the person double-clicks an event → its effects window opens, as on a
  free machine
- **TEST** t2214 (`src/apps/kt-app/src/layout_tests.rs`); live `formula`, `lessons`, `forms`
