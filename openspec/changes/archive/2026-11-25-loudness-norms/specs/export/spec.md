## MODIFIED Requirements

### Requirement: Loudness is measured in the export pass, without master gain
Export SHALL report the integrated loudness, the true peak and the LRA (BS.1770/EBU R128)
of the finished mix AFTER the project's program stage, measured as capability `audio-render` states it, and judge them
against the project's loudness target and ceiling. The player's monitoring gain MUST NOT exist in the project model and
MUST NOT enter the measurement. The project's program stage (a gain and a true-peak limiter, the owner's decision of
2026-11-22, B6) IS part of the mix and of the measurement. The report SHALL show each value with its unit, the project's
norm and a state (within, within tolerance, outside), per-channel peak, loudness, balance and silence, and timed
findings (over the ceiling, a silent channel, an imbalance), each moving the playhead to its place; render presets SHALL carry no target.
Why: the number in the preview and the number in the file match BY CONSTRUCTION only if one code counts both over the
same material; the owner — "на уровне проекта нормализация а не в пресетах рендера"; T14 — "на цифры вообще внимание не
обратишь такие".

#### Scenario: Loudness hears the event level
- **WHEN** the same event is exported at 0 dB and at −10 dB with no program stage
- **THEN** the difference is exactly 10 LU (±0.15), and the true peak matches the amplitude of the source
- **TEST** t406 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The master gain is not in the measurement
- **WHEN** the player's monitoring volume is changed and the export is repeated; then a program stage of +3 dB, under its ceiling, is added
- **THEN** there is no monitoring-gain field in the project and the number is unchanged; with the stage the number is 3 LU higher
- **TEST** t407 (`src/libs/kt-engine/tests/export_loudness.rs`)

#### Scenario: The meter that export measures with
- **WHEN** the meter receives a calibration sine / an inter-sample overshoot / the same signal cut differently
- **THEN** 1 kHz at full scale = 0.0 LUFS; the true peak sees the overshoot between samples; the cutting does not change the result; the sums are in f64, because f32 accumulation over 10 minutes drifts by 0.37 LU (a guard of the APPROVAL, not of the code)
- **TEST** t403, t403b, t405 (`src/libs/kt-fx/tests/loudness.rs`) — the meter itself belongs to capability `audio-render`

#### Scenario: One loud channel and one silent stretch are found against the project target
- **WHEN** a stereo project with the left channel +12 dB over the right for 3 s, the right channel silent for 2 s, and a passage over the ceiling is exported twice with the target Streaming
- **THEN** the report lists an imbalance span of 3 s, a silent-channel span of 2 s and an over-ceiling span at their times, judged against −14 LUFS / −1 dBTP; both reports are equal
- **TEST** t3921 (`src/libs/kt-engine/tests/export_loudness.rs`)

### Requirement: The render's loudness stands against its norms, per channel
The loudness measured on the export pass SHALL also give each channel's level, over the blocks the
summed measurement's gates keep, and each channel's true peak. The render result SHALL show
integrated loudness, true peak, loudness range, the two channels and, where the document has one, what the program
stage did, each beside its norm (the project's target ± 1 LU; the project's ceiling; ≤ 20 LU; channels within 3 LU; the
limiter at most 1 LU), a value out of its norm in amber, and a level 3 LU off or louder than −9 LUFS, a peak over the
ceiling, a silent channel or channels 6 LU apart in the error colour; the title counts what is out, and what is out is a
line of the render log. kt-cli export SHALL print the same.
Why: the owner, `v26.10` line 56 — "на цифры вообще внимание не обратишь такие. а еще не понятно что
является нормой у каждого параметра"; a mix with one channel silent measured exactly −23 LUFS.

#### Scenario: Each channel on its own
- **WHEN** a −20 dBFS 1 kHz tone is measured in both channels, with the right one 10 dB down, and with the right one silent
- **THEN** a balanced channel reads 3 LU under the summed value and −20 dBTP; 10 dB down reads 10 LU down with its own true peak −30 dBTP; a silent channel has no level and no peak while the summed value is the left channel's
- **TEST** t3106 (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: The result table judges each value
- **WHEN** the result rows are built for a −23 LUFS balanced mix, a −14 mix against a streaming project and against a −23 one, a −0.4 dBTP peak, a silent right channel, channels 10 and 4 LU apart, 2 and 4 LU off and −6 LUFS, a limiter that took 0.4 and 1.6 LU; the table of a bad case is drawn
- **THEN** a mix at its project's target is plain on every row and the −14 one is bad against −23; the peak, the silent channel, 10 LU apart, 4 LU off and −6 LUFS are bad; 4 LU apart, 2 LU off and 1.6 LU taken are amber; each row prints its norm, the title counts two out, the channels row reads "L -24.0 · R silent LUFS" and carries the bad mark
- **PATH** File → Render… → Render… → the render finishes → under the summary "Sound — N outside the norm" and the table: Integrated, True peak, Range, Channels L / R, each with its norm in grey beside it → a render with the right channel silent shows "R silent" in red and, under the table, the stretch where it is silent; a click on it moves the cursor there
- **TEST** t3107 (`src/apps/kt-app/src/layout_tests.rs`), live `render_loudness` (`src/scripts/live/render_loudness.py`)
