# Spec Delta

## MODIFIED Requirements

### Requirement: The event level is mixed where the fades live
The opacity of a video event SHALL be multiplied by the alpha of the layer at the same place as the fade:
an ordinary layer — by scaling the alpha, a solo fade — by multiplication, and for a dissolve pair the levels SHALL be
blended with the same weight as the picture. Level 1.0 is the identity. The level line in the UI is one for audio
(gain from silence to +6.02 dB) and for video (opacity); the corner of the fade takes priority over the line.
Why: otherwise a crossfade between clips of different transparency would jump, and the fades on audio and video would
look different (the owner's requirement).

#### Scenario: The level of a pair and of a solo
- **WHEN** a clip with a level, a solo fade with a level and a pair with different levels are rendered
- **THEN** the layer is scaled, the solo fade is multiplied, the pair blends the levels by the weight of the progress; 1.0 is the identity
- **TEST** t174 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: One level line for both kinds
- **WHEN** the level line is pulled on audio and on video
- **THEN** the rail range is preserved, the level↔gain round trip is exact, the corner of the fade has priority on both kinds
- **TEST** t176 (`src/apps/kt-app/src/layout_tests.rs`)
