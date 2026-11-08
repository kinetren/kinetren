## ADDED Requirements

### Requirement: An audio build's work grows linearly with the project
`AudioRenderer::new` SHALL do work linear in the project (up to sorting): a transition's ends, its
engine events by original id and edge, a link's leader, a reel's revision and a speed curve's
profile are each found or computed once, never by a scan or a rebuild per element of another
structure. The guard counts, never times (t1671): the elements whole-structure walks touched
(`kt_time::cost`) and the bytes allocated on the building thread, at a generated project ×1, ×4 and
×16 along each axis; ×16 may cost at most 32× the ×1 work.
Why: the class of found-defects row 151 (`docs/engine-audit.md` M2) — an O(whole) helper in a loop
over its parts — is invisible on hand-made projects, and every edit pays every build (2 audio and
2–3 video builds per edit, rows 110, 111, 117, 150).

#### Scenario: Events with transitions
- **WHEN** 24, 96 and 384 linked A/V pairs with shaped speed curves stand butted with a crossfade at every joint
- **THEN** the audio build's walked elements and allocated bytes grow at most 8× and 32×
- **TEST** t2062 (`src/libs/kt-engine/tests/build_cost.rs`)

#### Scenario: A long shaped signed speed curve
- **WHEN** one pair carries a shaped speed curve crossing zero over 8, 32 and 128 seconds
- **THEN** the build's work grows at most 8× and 32× (the former code: 16× and 256× in segments laid out)
- **TEST** t2063 (`src/libs/kt-engine/tests/build_cost.rs`)

#### Scenario: A chain of linked curves
- **WHEN** 12, 48 and 192 events each link their gain curve to the previous event's
- **THEN** the build's work grows at most 8× and 32×: the leaders resolve first, each found once
- **TEST** t2067 (`src/libs/kt-engine/tests/build_cost.rs`)
