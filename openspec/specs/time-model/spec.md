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

### Requirement: A reading of the warp map for a whole curve is linear in the curve
A function that reads the warp map at many points of one speed profile (the direction-change
knots `sign_knots`, the span `span_over`) SHALL lay the profile's segment lists and their integrals
out once per call and read them by halving, never through a per-point call that lays them out
again. The readings SHALL be the former per-point readings bit for bit (invariant 11). A curve
reading (`curve_value`) SHALL find its segment by halving on the sorted keys. The work is counted
by `kt_time::cost` (`warp_segments`, `curve_walks`), a thread-local meter of the elements a walk
over a whole structure touches; a new walk over something a project can make long counts there.
Why: a shaped speed curve is sampled into a point every 5 ms (`rate_profile_keys`), and
`sign_knots` asked three readings per point that each rebuilt the whole segment list: 393–488 ms of
every build of the owner's project (found-defects row 151) against 1 ms on the heavy showcase.

#### Scenario: Bit for bit
- **WHEN** the knots and the span of flat, straight, zero-crossing, zero-holding, trimmed, unsorted and 3000-point swinging profiles are computed
- **THEN** they equal the former per-point readings (frozen inside the test) to the bit
- **TEST** t2060 (`src/libs/kt-model/src/warp.rs`)

#### Scenario: Linear in the curve
- **WHEN** the knots and span of a sampled swinging curve of 500 and of 8000 points are computed
- **THEN** the segments laid out grow at most 16×; the former readings laid out 256×
- **TEST** t2061 (`src/libs/kt-model/src/warp.rs`)

### Requirement: A speed profile is compiled once per content
A speed curve's profile (the sampled straight points of `rate_profile_keys`, the points in material
fractions the audio chain reads, the clip's warp map with its integral) SHALL be computed once per
CONTENT and memoized: the key is everything that determines it — the curve's keys (tick, value bits,
shape and its parameter bits, the interned segment formula), the layer formula the sampling reads,
the material window, the reverse, the base speed and the blend — compared whole on a hit, never a
hash alone. The store SHALL be bounded (two generations of a fixed number of points), shared by every
thread, and invisible to determinism: a hit is the value the function computes for that key, the
same bits with and without the store (invariant 11). The sampling SHALL read the curve by a cursor
and the layer formula compiled, giving the former sampling's bits. Work is counted by
`kt_time::cost` (`profile_samples`, `formula_parses`, `curve_walks`).
Why: every `clip_warp` resampled and re-laid the profile — per piece in `clip_slice`, per trim
helper, per morph pass, in every build of every consumer (2 audio + 2–3 video builds per edit): 96 020
and 166 968 segments per audio/video build of the relinked copy of the owner's project, for curves no
edit had touched.

#### Scenario: The sampling is the former bits
- **WHEN** straight, shaped (every shape, a segment formula, a trimmed window), formula-layer, failing-layer and linked speed curves are sampled
- **THEN** the points equal the former sampling (frozen inside the test) to the bit
- **TEST** t2103 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: A sampled profile costs its points
- **WHEN** 60 s of a segment-formula speed curve and of a layer formula over 1801 keys are sampled on the 5 ms grid
- **THEN** the curve walks stay within twice the points plus keys, one parse per profile, and the bytes allocated per point stay within three keys' size
- **TEST** t2102 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: A memoized profile equals a fresh one
- **WHEN** the warp maps and audio profiles of shaped, formula-layer, straight, curveless, reversed and trimmed slowed clips are asked fresh, then twice through the store
- **THEN** all three readings are the same bits, and a hit samples no profile and lays out no map
- **TEST** t2104 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: The key is everything that determines the profile
- **WHEN** after a clip's map is stored, a value moves by one ulp, a tension, a segment formula, a key tick, a layer formula, the material entry, the length, the reverse or the base speed changes
- **THEN** the store returns the changed clip's own fresh map, never the stored one
- **TEST** t2105 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: A rebuild samples only what an edit changed
- **WHEN** a project of 40 slowed A/V pairs with distinct shaped speed curves is built (effective clips, audio and video renderers) three times, then once more after one pair's curve is edited
- **THEN** a rebuild of the same content samples no profile and lays out fewer map segments, and the edited build samples at most 1/20 of the first build's points
- **TEST** t2106 (`src/libs/kt-engine/tests/build_cost.rs`)
