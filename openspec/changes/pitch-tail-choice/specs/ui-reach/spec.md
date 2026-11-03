## ADDED Requirements

### Requirement: The pitch panel says what happens to the echo before it
The pitch effect's panel SHALL show the choice "Echo of the effects before the pitch" with the two
options "cut at the event's end" and "rings on after the event", in the person's language, with a hint
that says what each does (cut: the joins are seamless, the echo of a reverb before the pitch ends with
the event; rings on: the echo continues after the event as without the pitch, the joins stay as they are). A choice SHALL be one undoable edit of the slot.
Why: the owner asked for both behaviours in his own terms; a switch named in the engine's words would
repeat the unanswerable question.

#### Scenario: The switch is found in the pitch panel and changes the slot
- **WHEN** the FX window of an event with a pitch is drawn, the echo choice is set to "rings on", then undone
- **THEN** the panel shows the label, both options and the hint in every catalog; the slot's row becomes "ring" in one undo step and returns to "cut" on undo
- **PATH** double-click the event → the FX window → click the Pitch Shift slot → under the semitones the line "Echo of the effects before the pitch: cut at the event's end"; hovering it puts the hint in the status line → open it, pick "rings on after the event" → the line shows the new choice → Ctrl+Z → back to "cut"
- **TEST** t1337 (`src/apps/kt-app/src/pitch_tail_tests.rs`)

#### Scenario: Live — the switch in the real program
- **WHEN** the real program opens our fixture project with a pitch on an audio event, the FX window is opened by a double click, the echo choice is switched to "rings on" and the project saved
- **THEN** a screenshot per step shows the window, the choice before and after; the saved `.kt` carries `"echo_before": "ring"` and is format 3
- **PATH** double-click the audio event → the FX window (Delay, Pitch Shift) → click the Pitch Shift slot → the echo line "cut at the event's end" → open it → pick "rings on after the event" → Ctrl+S
- **TEST** t1338 (`src/scripts/live/pitchtail.py`)
