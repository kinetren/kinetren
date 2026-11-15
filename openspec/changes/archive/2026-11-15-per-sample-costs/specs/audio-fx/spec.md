## ADDED Requirements

### Requirement: An audio kernel's inner loop does no work it can take once
An audio kernel SHALL NOT recompute per sample, per sub-sample or per channel a value that is the same
over a larger unit, nor wrap a ring by a division, nor copy its chunk per call, where the cheaper form
is exact: `10^x` through `math::exp10f` (Phaser's sweep, the compressor's gain, the distortion's drive);
one biquad tune per channel pair (EQ, filter); delay lines, combs, allpasses and pre-delays wrapping by
compare-and-reset; the warp's and a strength's chunk buffers kept; the camera microphone's anti-image
filter summing only its kept taps, its gusts once per cell, its rows renewed in place; VHS Tape's sound
asking its dropout once a frame and renewing its deck in place; the vocoder's window pre-scaled and its
overwritten frame not zeroed. Phaser's per-sample sweep stays per sample (the owner's sound decision).
Every audio kind keeps the bytes it gave before, on the CPU's, the baseline and the general path.
Why: found-defects rows 128, 350, 351, 353–359 (per-sample-costs).

#### Scenario: The inner loops keep their bytes
- **WHEN** the warp under a moving speed (both resampler modes), a layer strength, the tail twin and loop laps of a pitched event (WSOLA and vocoder), Pitch's vocoder curved, a track stage keyed by another track, a nested reel, Phaser/Chorus/Flanger/Tremolo with curved rates, the filter, EQ, delay, compressor and Freeverb with curved rows render through the real renderer in odd slices
- **THEN** every digest equals the one printed on the tree before this change, on the CPU's, the baseline and the general path
- **TEST** t2680 (`src/libs/kt-engine/tests/per_sample_costs.rs`)

#### Scenario: exp10f is powf(10, ·) on every float
- **WHEN** every one of the 2³² `f32` bit patterns goes through `math::exp10f` and `math::powf(10, ·)`
- **THEN** the results are equal to the bit
- **TEST** t2682 (`src/libs/kt-plugin-api/tests/math.rs`)
