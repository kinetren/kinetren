# Spec Delta

## ADDED Requirements

### Requirement: Track properties — in the passport, static; curves on a track are rejected loudly
The level, opacity, blend mode, and switches of a track SHALL be typed passport properties with clamping, addressable by the track id; the properties window and the track FX editor SHALL read the limits from the passport. A curve on a track property or on a parameter of a track chain SHALL be rejected by validation and by the interface with a reason ("the time domain of the track is the next work"), and not silently accepted in the domain of material.
Why: a track has no material, its key is the timeline tick, and this is a second automation domain (F-AUT-003); the place is established now so that the next change does not break the format.

#### Scenario: The track's passport and a loud refusal of a curve
- **WHEN** the properties of a track are read by address; a key is set on the track level by the command and from the widget
- **THEN** the limits and types match the spec; the command is rejected with a reason, an error appears in the status bar, the model is untouched
- **TEST** t730 (`src/libs/kt-model/tests/track.rs`), t744 (`src/apps/kt-app/src/layout_tests.rs`)
