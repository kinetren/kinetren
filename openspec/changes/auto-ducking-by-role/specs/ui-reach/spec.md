## ADDED Requirements

### Requirement: Auto-ducking is one section of the background track's window
The audio track's window SHALL hold the section "Auto-ducking (background under content and titles)" with "During
content: down by N dB, fade" and its key list (tracks ticked, roles ticked), and "During titles: Raise by N dB / Mute /
Leave"; an audio track's header menu SHALL open it; roles SHALL be set from the track header, the event menu and the
track window; the automatic gain SHALL be drawn as a band under the background events' level line; "Convert to volume
curve" SHALL bake it in one undo step; nothing SHALL be computed at rest.
Why: rules 21, 23, 24; the owner's N3 and B5.

#### Scenario: Set up the sections by mouse
- **WHEN** the person opens Auto-ducking on the music track, ticks two speech tracks, keeps −12 dB, picks "Raise by 6 dB" for titles
- **THEN** shaded bands appear on the music events: dips under speech, rises over the titles
- **PATH** right-click the music track header → "Auto-ducking…" → the track window opens at the section → in "During content" tick "Speech 1" and "Speech 2" → in "During titles" pick "Raise by" 6 dB → close → the music events show dips under speech and rises over each title
- **TEST** t3915 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `ducking_roles`

#### Scenario: Convert to volume curve
- **WHEN** the person presses "Convert to volume curve" in the section
- **THEN** the music events carry gain keys where the band was, both rules are off, one Ctrl+Z restores them
- **PATH** in the Auto-ducking section click "Convert to volume curve" → the bands become a level curve with keys on the music events → Ctrl+Z → the bands are back
- **TEST** t3916 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Nothing runs at rest
- **WHEN** a project with Auto-ducking is paused and the mouse moves over the bands for 2 s
- **THEN** no analysis job starts and the frames replay the cached band mesh
- **PATH** pause on a project with a ducked background → move the mouse over the music events → the bands stay as drawn, nothing recomputes
- **TEST** t3917 (`src/apps/kt-app/src/layout_tests.rs`)
