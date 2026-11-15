## ADDED Requirements

### Requirement: The noise generator hashes a row when its held value changes
The audio noise generator SHALL hash row k of pink or brown noise only when `n >> k` changes, summing
the rows in their order, so every colour, seed, stereo choice and start gives the bytes it gave before.
Why: found-defects row 360 (per-sample-costs).

#### Scenario: Noise keeps its bytes
- **WHEN** white, pink and brown noise with two seeds and both stereo choices are read from four starts, aligned and not to a row's hold
- **THEN** each digest equals the one printed before the change
- **TEST** t2681 (`src/plugins/generators-audio/tests/sound.rs`)
