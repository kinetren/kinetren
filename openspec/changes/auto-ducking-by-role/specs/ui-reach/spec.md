## ADDED Requirements

### Requirement: Roles and Auto-ducking are reached where the person stands
A role SHALL be set from the audio track header's menu, the audio event's menu and the track window, and be seen
as the waveform's colour; "Auto-ducking…" SHALL be an item of an audio track's header menu opening the track window
at its section, named by the industry word first; the reduction SHALL be drawn under the background events' level
line; nothing SHALL be drawn or computed at rest.
Why: rules 21, 23, 24; the owner's N3.

#### Scenario: Tag speech, duck the music, see it
- **WHEN** the person tags the speech track Dialogue, opens Auto-ducking on the music track, ticks Dialogue and sets −12 dB
- **THEN** the music events show a shaded reduction band under each speech event; the section's values are remembered for the next track
- **PATH** right-click the speech track header → "Role" → "Dialogue" → its waveform turns the Dialogue colour → right-click the music track header → "Auto-ducking…" → the track window opens at "Auto-ducking (background under speech and titles)" → tick "Dialogue", type −12 in "Reduce by" → close → shaded bands appear on the music events under every speech event
- **TEST** t3915 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `ducking_roles`

#### Scenario: Convert to volume curve
- **WHEN** the person presses "Convert to volume curve" in the section
- **THEN** the music events carry gain keys where the band was, the compressor is off, one Ctrl+Z restores it
- **PATH** in the Auto-ducking section click "Convert to volume curve" → the bands become a level curve with keys on the music events → Ctrl+Z → the bands are back
- **TEST** t3916 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Nothing runs at rest
- **WHEN** a project with a ducked background is paused and the mouse moves over the bands for 2 s
- **THEN** no analysis job starts and the frames replay the cached band mesh
- **PATH** pause on a project with a ducked background → move the mouse over the music events → the bands stay as drawn, nothing recomputes
- **TEST** t3917 (`src/apps/kt-app/src/layout_tests.rs`)
