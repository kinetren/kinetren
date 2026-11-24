## ADDED Requirements

### Requirement: Beats, cuts and motion are drawn where they are heard and seen
A switch on a track header SHALL draw the measures of the track's events — beats and bars on a waveform, cuts and a motion
curve on a filmstrip — through each event's own map, and while drawn the beats and cuts SHALL be snap targets found
without walking them all. The switch SHALL be off by default and remembered; a hover frame SHALL replay the mesh.
Why: the layer is useful when it is seen at the place where the cut is made.

#### Scenario: Snap to a beat
- **WHEN** the switch is on over a music event at 120 BPM and a picture event's edge is dragged near a beat, then the switch is turned off and the drag repeated
- **THEN** the edge lands on the beat's tick and the snap line shows; with the switch off it does not
- **PATH** press "Beats" on the sound track's header → ticks appear on its waveform → drag the right edge of the picture event above near a tick → it jumps to the tick and a line shows → press "Beats" again → drag → no jump
- **TEST** t4217 (`src/apps/kt-app/src/layout_tests.rs`), planned live scenario `beats` (t4218)

#### Scenario: At rest with everything shown
- **WHEN** the heavy showcase is paused with both switches on and Find moments open, and the mouse moves over the tracks
- **THEN** no measure is computed, no overlay mesh is rebuilt, and only hover frames are drawn
- **PATH** switch on "Beats" and "Cuts and motion" → open Find moments → pause → move the mouse over the tracks → nothing but the hover changes
- **TEST** t4228 (`src/apps/kt-app/src/frame_cost_tests.rs`)
