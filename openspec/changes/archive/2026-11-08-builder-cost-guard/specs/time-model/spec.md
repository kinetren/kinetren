## ADDED Requirements

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
