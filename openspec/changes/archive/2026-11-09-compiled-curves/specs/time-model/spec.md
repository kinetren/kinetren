## ADDED Requirements

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
