# Spec Delta

## ADDED Requirements

### Requirement: Track as an implicit reel in the plan; blend modes as a pair; video mute/solo
A video track with a non-empty chain, a level ≠ 1, or a blend mode ≠ Normal SHALL enter the plan as a single nested layer: the composite of its events (transitions, overlaps, fades, event levels — inside) by the same code as a nested reel (raster — the parent's canvas, time — the parent's, the step — the maximum over its events), then the track chain, then alpha × level and blending downward with the chosen mode. A track without these properties SHALL be flattened into layers as before — byte for byte and at the same cost. A silenced (mute / not in solo) video track MUST NOT be composited, but its prefetch order SHALL be preserved. The blend mode SHALL be computed as `out = lerp(below, blend(below, above), α_above × level)`; Normal — the former alpha_over, byte for byte; the set: Normal, Multiply, Screen, Add, Subtract, Darken, Lighten, Overlay, Difference; the CPU kernel and the WGSL are edited as a pair by the tolerance class of transitions. A hole on a track is transparency of its composite, not an operand. The memo of the track composite SHALL be keyed by (track, track revision, tick) by the same mechanism as for a reel; the morpher remains per-clip and stands BEFORE the track stage.
Why: a chain appended to every event gives a different picture on transitions and overlaps and cannot do a vignette "over everything" (F-MPH-005/007); a mode on a track whose hole is an operand of the ink darkens the lower layers in Vegas (F-CMP-003); a mute that loses the order would give a black frame on unmuting.

#### Scenario: Without a condition — the earlier bytes and cost; with a chain — the track's composite
- **WHEN** a project without track properties; then a vignette on a track with a transition between two events; a second composite of the same tick; export
- **THEN** t11/t67/t184/`GOLDEN_VIDEO` byte for byte and the number of layers is as before; the vignette covers the transition entirely and is not equal to a vignette appended to each event; the second composite falls into the track's memo; preview == export
- **TEST** t736 (`src/libs/kt-engine/tests/track_video.rs`)

#### Scenario: Blend modes against the formula, as a pair
- **WHEN** each mode is applied to a pair of tracks with known colors; the track level is 0.5; on a track with Multiply — a hole; the same plans on GPU
- **THEN** the result equals the analytic formula; Normal is byte for byte as before; 0.5 — half the way to the result of the mode, not the mode over a semi-transparent one; in the hole the lower layers are visible without darkening; GPU conformance within the tolerance class of transitions
- **TEST** t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Video mute and solo
- **WHEN** a track is silenced; then two of three are in solo; then mute is cleared
- **THEN** the frame does not include the silenced track, `wanted_frames` contains its frames; in solo exactly two are composited; after clearing mute the frame is complete without a cache miss
- **TEST** t735 (`src/libs/kt-engine/tests/track_video.rs`)
