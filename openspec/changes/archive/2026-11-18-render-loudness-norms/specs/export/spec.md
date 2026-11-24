## ADDED Requirements

### Requirement: The render's loudness stands against its norms, per channel
The loudness measured on the export pass SHALL also give each channel's level, over the blocks the
summed measurement's gates keep, and each channel's true peak. The render result SHALL show
integrated loudness, true peak, loudness range and the two channels each beside its norm (−23 LUFS
EBU R 128 · −14 streaming; ≤ −1 dBTP; ≤ 20 LU; channels within 3 LU), a value out of its norm in amber
and a peak over the ceiling, a silent channel, channels 6 LU apart or a level louder than −9 LUFS in the
error colour. Nothing SHALL be applied to the audio.
Why: the owner, `v26.10` line 56 — "на цифры вообще внимание не обратишь такие. а еще не понятно что
является нормой у каждого параметра"; a mix with one channel silent measured exactly −23 LUFS.

#### Scenario: Each channel on its own
- **WHEN** a −20 dBFS 1 kHz tone is measured in both channels, with the right one 10 dB down, and with the right one silent
- **THEN** a balanced channel reads 3 LU under the summed value and −20 dBTP; 10 dB down reads 10 LU down with its own true peak −30 dBTP; a silent channel has no level and no peak while the summed value is the left channel's
- **TEST** t3106 (`src/libs/kt-fx/tests/loudness.rs`)

#### Scenario: The result table judges each value
- **WHEN** the result rows are built for a −23 LUFS balanced mix, a −14 streaming mix, a −0.4 dBTP peak, a silent right channel, channels 10 and 4 LU apart, −19 and −6 LUFS; the table of the bad case is drawn
- **THEN** the good mixes are plain on every row; the peak, the silent channel, 10 LU apart and −6 LUFS are bad; 4 LU apart and −19 LUFS are amber; each row prints its norm, the channels row reads "L -24.0 · R silent LUFS" and carries the bad mark
- **PATH** File → Render… → Render… → the render finishes → under the summary the Loudness table: Integrated, True peak, Range, Channels L / R, each with its norm in grey beside it → a render with the right channel silent shows "R silent" in red
- **TEST** t3107 (`src/apps/kt-app/src/layout_tests.rs`)
