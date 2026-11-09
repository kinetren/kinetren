## ADDED Requirements

### Requirement: An audio kernel standing still takes its constant path, with the same bytes
An audio kernel SHALL NOT redo per-sample or per-block work whose inputs have not changed, where skipping
it is exact: the filter and the EQ re-tune their biquads only when the bits of the tune's inputs change;
distortion, bitcrusher, stereo width and the automated gain take one set of values for a block whose
smoothers stand still; the compressor below `below_threshold(thr)` (the threshold's level 0.0087 dB lower)
takes the make-up gain without its `log10` and `pow`; the camera microphone's resampler reads doubled
rings instead of shifting its histories. Every audio kind, at rest and moving, gives the bytes it gave
before these variants, on the specialized, the general (`cpu::with_general`) and the baseline path, and a
variant that is taken costs measurably less than the general path in the same run. Phaser's per-sample
filter stays (row 128, the owner's decision).
Why: the owner, 2026-11-09 (kkt-26.11.09 I9); engine-audit §3.2 "Audio DSP per sample"; found-defects
row 129 (the compressor's `log10` per sample).

#### Scenario: Every kind keeps its bytes at rest and moving
- **WHEN** every audio kind runs quiet material, material with silent gaps, its knobs moved to the middle and back halfway, a switch to its heavy settings, and the camera's two decimating codecs, specialized, general and baseline
- **THEN** every digest equals the one printed on the tree before the variants
- **TEST** t2171 (`src/libs/kt-engine/tests/kernel_specialization.rs`)

#### Scenario: Below the safe level the compressor's gain is the make-up gain to the bit
- **WHEN** for 3 200 thresholds from −100 to +20 dB the 4 096 envelopes just under `below_threshold` and 4 096 spread below it go through the general formula
- **THEN** the overshoot is +0 and the gain equals the make-up gain's bits; `10^(−0)` is exactly 1
- **TEST** t2172 (`src/plugins/fx-audio-dynamics/tests/below_threshold.rs`)

#### Scenario: The variants pay
- **WHEN** the compressor on quiet material, the filter and the heavy EQ at rest are timed against the reference kernel on the specialized and the general path alternately
- **THEN** each specialized path costs at most 70 %, 80 % and 75 % of its general path (measured 40 %, 54 %, 44 %)
- **TEST** t2176 (`src/libs/kt-engine/tests/kernel_specialization.rs`)
