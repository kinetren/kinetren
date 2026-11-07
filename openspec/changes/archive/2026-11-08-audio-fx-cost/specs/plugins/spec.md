## ADDED Requirements

### Requirement: An audio kernel's block runs at the CPU's level, with the same bytes
An audio plugin state's `process` SHALL run its per-block body through `kt_plugin_api::fast!`: compiled
a second time with AVX2 enabled and taken where the CPU reports AVX2, the baseline x86-64 build
otherwise (and on every other architecture as built). The two SHALL give the same bytes: no FMA in the
feature set, no reordered float sums, the rounding functions exact by IEEE 754. `KT_SIMD=0` forces the
baseline path for the process; `cpu::with_baseline` does so for one thread (the reference of t1981). A
per-sample helper the body calls is `#[inline]`, or it keeps the baseline code.
Why: SSE2 has no instruction for `floor`/`ceil`/`round`/`trunc`: on the shipped build each one on a
per-sample path was a call into the C library through the PLT — ten of the 21 audio kernels, fifteen
call sites in VHS Tape's sound — and the loops around them could not use wider registers.

#### Scenario: Both paths give the same bytes
- **WHEN** every audio kind's digests are taken on the CPU's path and inside `cpu::with_baseline`
- **THEN** both equal the digests printed before the paths existed
- **TEST** t1981 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)

### Requirement: A cheaper form of a frozen function is that function to the bit
A replacement of a frozen-math call on a per-sample path by a cheaper form SHALL return the same bits on
every input, proved exhaustively where the input is one `f32`. `math::powf_neg_half(x)` is
`powf(x, -0.5)`: `1 / sqrt(x)` in `f64`, and `powf` itself outside the positive finite numbers and where
the `f64` value lies within 2⁸ `f64` ulps of a boundary between two `f32` roundings.
Why: VHS Tape's Hi-Fi compander took a `pow` per sample per channel, 17 % of the effect's cost; kt-fx
CLAUDE.md: a rounding trick is closed by an exhaustive test.

#### Scenario: The replacement equals pow on every float
- **WHEN** `powf_neg_half` and `powf(x, -0.5)` are taken on every positive finite `f32`, zero, the infinities, NaN and a negative
- **THEN** their bits are equal on all of them
- **TEST** t1982 (`src/libs/kt-plugin-api/tests/math.rs`)
