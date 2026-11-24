## MODIFIED Requirements

### Requirement: Loudness is measured in the export pass, without master gain
Export SHALL report the integrated loudness, the true peak and the LRA (BS.1770/EBU R128)
of the finished mix AFTER the project's program stage, measured as capability `audio-render` states it, and judge them
against the project's loudness target and ceiling. The player's monitoring gain MUST NOT exist in the project model and
MUST NOT enter the measurement. The project's program stage (a gain and a true-peak limiter, the owner's decision of
2026-11-22, B6) IS part of the mix and of the measurement. The report SHALL show each value with its unit, the project's
norm and a state (within, within tolerance, outside), per-channel peak, loudness, balance and silence, and timed
findings (over the ceiling, clipped samples, a silent channel, an imbalance); render presets SHALL carry no target.
Why: the number in the preview and the number in the file match BY CONSTRUCTION only if one code counts both over the
same material; the owner — "на уровне проекта нормализация а не в пресетах рендера"; T14 — "на цифры вообще внимание не
обратишь такие".

#### Scenario: Loudness hears the event level
- **WHEN** the same event is exported at 0 dB and at −10 dB with the program stage at 0 dB and its limiter off
- **THEN** the difference is exactly 10 LU (±0.15), and the true peak matches the amplitude of the source
- **TEST** t406 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The master gain is not in the measurement
- **WHEN** the player's monitoring volume is changed and the export is repeated; then the program stage's gain is raised 3 dB
- **THEN** there is no monitoring-gain field in the project and the number is unchanged; with the stage raised the number is 3 LU higher
- **TEST** t407 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The meter that export measures with
- **WHEN** the meter receives a calibration sine / an inter-sample overshoot / the same signal cut differently
- **THEN** 1 kHz at full scale = 0.0 LUFS; the true peak sees the overshoot between samples; the cutting does not change the result; the sums are in f64, because f32 accumulation over 10 minutes drifts by 0.37 LU (a guard of the APPROVAL, not of the code)
- **TEST** t403, t403b, t405 (`src/libs/kt-fx/tests/loudness.rs`) — the meter itself belongs to capability `audio-render`

#### Scenario: One loud channel and one silent stretch are found against the project target
- **WHEN** a stereo project with the left channel +12 dB over the right for 3 s, the right channel silent for 2 s, and a passage over the ceiling is exported twice with the target Streaming
- **THEN** the report lists an imbalance span of 3 s, a silent-channel span of 2 s and an over-ceiling span at their times, judged against −14 LUFS / −1 dBTP; both reports are equal
- **TEST** t3921 (`src/libs/kt-engine/tests/export_loudness.rs`)
