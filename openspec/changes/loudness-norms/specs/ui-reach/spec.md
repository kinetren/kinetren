## ADDED Requirements

### Requirement: The loudness target is the project's, and matching is reached where the person stands
Project settings SHALL hold the loudness target (a standard or a number) and the ceiling; "Match loudness…" SHALL be
offered on selected events and a track header, and "Match the program" on the program stage in the meters
column, which shows its gain, the limiter's light and the measured value or "changed since measured"; the render
report SHALL judge against the project's target.
Why: the owner, B6 and T14.

#### Scenario: Set the target, match the dialogue, match the program
- **WHEN** the person sets Streaming in Project settings, selects the dialogue events → "Match loudness…" → Dialogue, then presses "Match the program" and renders
- **THEN** the dialogue gains change in one step; the program stage shows "+5.5 dB, measured −19.5 LUFS"; the render report reads "Sound — 0 outside the norm" against −14 LUFS
- **PATH** File → Project settings → Sound → pick "Streaming (−14 LUFS)" → close → click a dialogue event, Shift+click the last one → right click → "Match loudness…" → pick "Dialogue (−14)" → OK → the dialogue levels move → click the program stage under the meters → "Match the program" → after the bar fills it shows "+5.5 dB" → Render → the result reads "Sound — 0 outside the norm"
- **TEST** t3922 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `render_norms`

#### Scenario: An edit marks the program as changed
- **WHEN** after matching the program an event's level is raised
- **THEN** the program stage shows "changed since measured" until "Match the program" is pressed again
- **PATH** raise a music event's level line → the program stage under the meters shows "changed since measured" → click it → "Match the program" → the mark goes
- **TEST** t3923 (`src/apps/kt-app/src/reach_tests.rs`)
