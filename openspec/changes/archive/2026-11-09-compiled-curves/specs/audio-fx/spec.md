## ADDED Requirements

### Requirement: An LFO's phase is read from a prefix integral
A modulation node's LFO phase (the integral of its frequency curve over the clip position) SHALL be
read from a prefix integral compiled when the node is bound (`ParamIntegral`): the sum of every whole
segment before each one, added in the former walk's order, and the segment of the position found by
halving. The phase SHALL be `param_integral`'s bits at every position (invariant 11), and its cost
O(log points) a sample.
Why: `param_integral` walked the curve from its first point per sample (found-defects row 152); a
formula layer on the frequency resolves to a point per frame — 10 800 points on a 6-minute event.

#### Scenario: The prefix integral is the walk bit for bit
- **WHEN** empty, single-point, duplicate-position, unsorted and random curves of 2 to 3000 points are read before, on, beside and after every point and at random positions, with base 0 and 3.5
- **THEN** every reading equals `param_integral` to the bit
- **TEST** t2107 (`src/libs/kt-plugin-api/src/curves.rs`)

#### Scenario: The phase costs O(log points) a sample
- **WHEN** tremolo and auto-pan render 2 s under a 3000-point frequency curve
- **THEN** the integral steps stay within 16 a sample
- **TEST** t2108 (`src/libs/kt-engine/tests/audio_fx_registry.rs`)
