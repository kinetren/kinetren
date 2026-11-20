## ADDED Requirements

### Requirement: The export reports the picture beside the loudness
The export pass SHALL measure, from the frames it already reads back and without steering anything, black
spans, frozen spans where the timeline intends no hold, frames with more than 1 % of samples outside the legal
range, and spans with more than three large luminance flashes within one second, and SHALL report them with
their times beside the loudness; the same project SHALL give the same report on every run.
Why: the owner, v26.10 T15 — "для видео такой же оценки нет?"; invariant 11.

#### Scenario: Synthetic mistakes are reported, intended holds are not
- **WHEN** a project with 1 s of black, a missing-media placeholder frozen for 2 s, a still image for 3 s, a speed-0 hold, a passage with 25 % super-white and a strobing generator at 5 Hz is exported twice
- **THEN** the report lists the black span, the frozen span of the placeholder, the out-of-range span and the flashing span at their times; the still and the speed-0 hold are not reported; the two reports are equal
- **TEST** t3908 (`src/libs/kt-engine/tests/picture_check.rs`)
