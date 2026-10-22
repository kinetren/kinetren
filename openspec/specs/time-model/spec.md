# Time model

## Purpose

A single integer time axis of the engine and the project format: a tick of 1/90000 s, from
which the frame grids of all rates and the 48 kHz audio burst boundaries are derived exactly.

## Requirements

### Requirement: Unit of time — tick 1/90000 s, integer i64
All internal engine time and the project format SHALL be expressed in ticks (`Tick`, i64,
90 000 per second). Milliseconds and floating-point seconds are allowed only at the UI and
debug boundary. Negative ticks are valid and are divided with floor semantics.
Why: 90 kHz divides 24/25/30/60 fps and the audio burst step, so no conversion accumulates error.

#### Scenario: Frame grids of all rates are integer
- **WHEN** a frame grid is created for fps from {24, 25, 30, 60}
- **THEN** the frame step is an integer number of ticks (90000/fps), and a grid for an fps that does not divide 90000 is rejected with a panic
- **TEST** `fps_grids_divide_tick_hz` (`src/libs/kt-time/src/lib.rs`)

#### Scenario: Negative time rounds down
- **WHEN** a tick is negative and is converted to a frame or burst index
- **THEN** the index is computed by floor division (`div_euclid`), not by truncation toward zero
- **TEST** `negative_ticks_floor_correctly` (`src/libs/kt-time/src/lib.rs`)

#### Scenario: The range is half-open
- **WHEN** a tick range is checked for membership, intersection and length
- **THEN** the start is included, the end is excluded; an empty range intersects nothing
- **TEST** `range_semantics_half_open` (`src/libs/kt-time/src/lib.rs`)

### Requirement: Audio burst — 1024 frames at 48 kHz = 1920 ticks, conversion is exact
The audio burst SHALL equal 1024 frames at 48 kHz, that is exactly 1920 ticks;
tick↔sample SHALL be computed as `ticks * 48000 / 90000` (ratio 15/8) without
floating point. The constants 90000, 48000 and 1024 are consistent and MUST NOT be changed
separately.

#### Scenario: The burst boundary is one-to-one with the sample number
- **WHEN** a burst number is converted to a tick, and the tick back to a sample number
- **THEN** the result matches the original with no remainder
- **TEST** `audio_burst_step_is_exact` (`src/libs/kt-time/src/lib.rs`)

#### Scenario: 48 kHz frames are exact only for multiples of 8
- **WHEN** an audio frame count is converted to ticks and back
- **THEN** the identity holds if and only if the frame count is a multiple of 8 (ticks that are multiples of 15); clip boundaries live in ticks, and the engine floors them into frames exactly once
- **TEST** the `t4_*` family (`src/libs/kt-engine/tests/fx.rs`) and `src/libs/kt-engine/tests/audio_engine.rs` — boundaries are set as multiples of 8

### Requirement: kt-time has no dependencies and no float time in the API
The kt-time crate SHALL build without dependencies (except serde) and without FFmpeg/audio
hardware; the public API MUST NOT accept or return floating-point time, except the explicit
`*_secs_f64` functions for UI and debugging.

#### Scenario: Building the core without system libraries
- **WHEN** kt-time, kt-model, kt-fx, kt-cache and the kt-engine core are built
- **THEN** the build does not require the FFmpeg, clang and ALSA dev packages
- **TEST** `cargo test -p kt-time -p kt-model -p kt-fx -p kt-cache` on a machine without FFmpeg (the rule in the root CLAUDE.md, section "Build and tests")
