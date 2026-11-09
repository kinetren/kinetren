## ADDED Requirements

### Requirement: The peaks pyramid folds at the CPU's level
`PeakAcc::push` SHALL run through `fast!` and fold a block's run of frames into a fixed-size array; the
pyramid's values do not change.
Why: engine-audit §3.2 "Analysis": the pyramid pass runs on every first opening of a file (7.2 → 4.1 ms
per minute of audio).

#### Scenario: The pyramid keeps its values on both paths
- **WHEN** material past full scale with a NaN, infinities and negative zeros is folded whole and in chunks of 1, 777 and 1 024 frames, and on the baseline path
- **THEN** the pyramid's digest equals the one printed before the change
- **TEST** t2175 (`src/libs/kt-media/tests/peak_acc.rs`)
