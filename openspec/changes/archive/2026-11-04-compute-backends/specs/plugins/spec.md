## ADDED Requirements

### Requirement: One kernel text gives the CPU byte reference and the GPU kernel
A kernel written once in the common subset of Rust and WGSL (`kt_plugin_api::shade`,
`shared_kernel!`) SHALL give its CPU form with the text's `f32` computed as f64 and its WGSL form as
the same tokens. The CPU form SHALL be byte for byte the kind's CPU reference at every SIMD level the
host compiles its loop for (baseline, SSE4.1, AVX2, AVX-512) and at every number of row bands; the
WGSL form SHALL compile in the own-pass frame of a point-wise plugin and stay within the kind's
existing tolerance class of the CPU reference. Until the owner orders the migration the shipped
plugins SHALL keep their own kernels; the single text is a prototype no shipped path calls.
Why: today one effect is up to three texts (scalar, AVX2 twin, WGSL) on one core (C2 of
kkt-26.11.03). Rust neither contracts `a·b + c` into an FMA nor reassociates, and the subset's
builtins are exact, so one text compiled for any SIMD level is the same bytes; a pixel of a point-wise
kernel is a pure function of itself and its position, so bands of rows on scoped threads are the
same bytes (invariant 11). The WGSL form is f32, as the hand-written mirror is, so the tolerance
class does not move (invariant 7).

#### Scenario: The single-text Color Grade is the shipped kernel byte for byte
- **WHEN** Color Grade written once runs on a 257×67 frame crossing every byte value, at six
  parameter sets (identity, hue ±180, saturation 0 and 2, brightness ±1), compiled for the baseline,
  SSE4.1, AVX2 and AVX-512 (a level the CPU lacks falls back), on 1, 3 and 8 bands
- **THEN** every frame equals the shipped `grade` through the barrier loop byte for byte, and the
  WGSL form holds both functions and no Rust-only syntax
- **TEST** t1500 (`src/plugins/fx-video-color/tests/single_source.rs`)

#### Scenario: The single text's WGSL runs on the GPU within the kind's tolerance
- **WHEN** the WGSL form and the hand-written `color_grade.wgsl` are compiled in the own-pass frame
  `assembled_shaders` gives a point-wise plugin and run at four parameter sets on a 192×108 frame
- **THEN** the single text compiles, and both GPU frames are within `FrameTol::BILINEAR` (Color
  Grade's class in t181) of the CPU reference
- **TEST** t1501 (`src/libs/kt-gpu/tests/single_source.rs`)
