## ADDED Requirements

### Requirement: The mixdown weighs a constant run once
The mixdown of an event, with and without effects, SHALL split its frames into runs of one weight regime
— bounded by the fade windows, the overlaid windows, the lane's overlap boundaries and the FX tail edge —
and take a constant regime's weight (`fade_weight · overlap_at · gain`) once per run by the same
expression, keeping the per-frame expression in ramp runs. The order of the f32 sums does not change.
Why: engine-audit §3.2 "Mixdown weights": branches, a `sqrt` and a binary search per sample also where the
weight is exactly 1; invariant 11.

#### Scenario: The runs keep the mix's bytes
- **WHEN** an edit with gains, crossfades with handles in all four curves, a fade from and to nothing, two and three overlapping events, an overlaid transition, a reversed event, effects and a ringing tail renders in bursts and in slices of 777 frames, specialized and inside `cpu::with_general`
- **THEN** the digests equal the ones printed before the runs
- **TEST** t2173 (`src/libs/kt-engine/tests/kernel_specialization.rs`)

### Requirement: The loudness meter runs at the CPU's level and keeps its bits
`LoudnessMeter::feed` SHALL run through `fast!`, its true peak SHALL read a doubled ring (no rotate of the
tail per sample) and fetch its interpolator table once per feed; LUFS, LRA and dBTP keep their bits.
Why: engine-audit §3.2 "Analysis": the meter runs in every export and on every first opening of a file;
it took 50 ms per minute of audio, 25 after.

#### Scenario: The meter keeps its bits on both paths
- **WHEN** music-like material, a pause, a quiet stretch and an inter-sample-peak sine are measured whole and in slices of 1, 777 and 4 096 frames, and on the baseline path
- **THEN** the bits of LUFS, LRA and dBTP equal the ones printed before the change
- **TEST** t2174 (`src/libs/kt-fx/tests/kernel_specialization.rs`)
