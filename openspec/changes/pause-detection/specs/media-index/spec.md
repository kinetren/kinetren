## ADDED Requirements

### Requirement: The peak pyramid covers the file's last burst
The peak pyramid SHALL include the frames of the last, partial burst of a file; no block SHALL be made of samples past the
file's end. Peaks stored before SHALL be a miss and rebuilt.
Why: F-AUD-008 — up to 21 ms at the end of every file were never seen; the same defect has been open in LosslessCut since
2024.

#### Scenario: A sound in the last milliseconds
- **WHEN** a file of 10 s and 700 frames is silent but for a click 300 frames before its end
- **THEN** the pyramid's level 0 holds the click in its last blocks, its length is the file's length in blocks rounded up, and the waveform draws it
- **TEST** t4204 (`src/libs/kt-media/tests/index_db.rs`)
