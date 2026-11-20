## ADDED Requirements

### Requirement: The export measures each channel and reports audio findings against a chosen norm
The export pass SHALL measure, besides the mixed integrated loudness, true peak and LRA (unchanged byte for byte),
each channel's peak and integrated loudness, their balance, and SHALL report as timed findings the spans above the
chosen norm's true-peak ceiling, clipped samples, a channel silent at least 0.5 s while the other sounds, and a
balance beyond 6 LU for at least 1 s. The norm (EBU R128, ATSC A/85, Streaming, Podcast) SHALL be a render setting
of the text form; the same render SHALL give the same report.
Why: the owner, v26.10 T14 — "сильным неприятным gain на одном канале или … тишине у другого … не понятно что
является нормой".

#### Scenario: One loud channel and one silent stretch are found
- **WHEN** a stereo project with the left channel +12 dB over the right for 3 s, the right channel silent for 2 s, and a passage over −1 dBTP is exported twice with the norm Streaming
- **THEN** the report lists an imbalance span of 3 s, a silent-channel span of 2 s on the right and an over-ceiling span at their times; the mixed LUFS/dBTP/LRA equal the values before this change; both reports are equal
- **TEST** t3919 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: Per-channel numbers on calibration signals
- **WHEN** the meter receives a −20 dBFS 1 kHz sine on the left only, then on both channels
- **THEN** the left loudness is −23.0 LUFS (±0.1) per channel, the right reports no loudness (below the gate) on the first signal; the mixed reading is unchanged against t403
- **TEST** t3920 (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: The norm travels in the render settings
- **WHEN** a preset with `loudness_norm=ebu_r128` is written, read by kt-cli and by the window, and a preset with an unknown norm id is read
- **THEN** both readers judge with EBU R128; the unknown id is listed and kept verbatim
- **TEST** t3921 (`src/libs/kt-engine/tests/render_settings.rs`)
