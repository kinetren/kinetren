## REMOVED Requirements

### Requirement: Target cutting rules
**Reason**: its veto "a clip with its own or appended time warp MUST NOT be cut" and the scenario "A warped clip is not cut" lost their premise in R.39 (a piece's entry point is the integral of the speed) and are revoked by the owner's claim (ktprompts v26.10.18 item 3); restated below.
**Migration**: every other rule and scenario moves verbatim into "Target cutting rules — a warped clip is cut like any other"; `t238_warped_clip_is_never_sliced` is inverted, not deleted.

## ADDED Requirements

### Requirement: Target cutting rules — a warped clip is cut like any other
When cutting, the boundary SHALL snap to the material grid (a frame for video, 8 samples for audio; for reversed the count runs from the end of the body), then be pushed out of forbidden zones (transition windows, neighbours' overlaps) only inward; a stub shorter than the minimum collapses to the edge of the body; a window shorter than the minimum has no effect. A clip with its own or appended time warp SHALL be cut like any other target: every piece's entry point is the integral of the speed (the one rule of a piece's entry point), and a patch that carries a time warp acts only in its window — the pieces outside the window keep the material, the speed and the curves they had without the layer. A clip covered entirely is not cut and keeps its id. Cut points SHALL be shared by all layers (morphers do not cut each other's pieces). The rule for a piece's entry point is one per project, and an ordinary split of an event goes through it too.
Why: cutting breaks not the target but its neighbours (the Vegas order "on top is the one that started later", the crossfade ramp from geometry); a linear `source_in` of a stretched clip points in the wrong place.
Revision (ktprompts v26.10.18 item 3, "childish excuses"): the veto "a warped clip is patched whole"
(R.35 finding 2, R.37) was written when a piece's `source_in` was computed linearly; since R.39 it is the
integral of the speed (t270–t270d), so the veto had no reason left, and it made a layer with slowed slow
the WHOLE event and a source swap over part of a slowed event replace its whole body.

#### Scenario: Partial coverage — three pieces with exact material
- **WHEN** a block covers the middle of a clip (direct and reversed)
- **THEN** three pieces result; the material of each matches to the tick with a manual cut
- **TEST** t233 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: Degenerate cases and forbidden zones
- **WHEN** a clip is covered entirely; the window is microscopic; a video patch over audio; the boundary falls into the crossfade window
- **THEN** the entirely covered clip is not cut and keeps its id; a micro-window does nothing; a video patch does not touch audio; the boundary sticks to the edge of the zone
- **TEST** t236 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: A warped clip is cut at the integral of its speed
- **WHEN** an audio clip and a linked video clip with their own slowed reverb (constant 0.8×, then a speed curve 0.5→1.5, then reversed) are covered in the middle by a layer with a source swap; and a plain clip is covered in the middle by a layer whose patch carries slowed reverb at 0.5×
- **THEN** the warped clips are cut into three pieces whose entry points are the integral of the speed (the seams have no material gap, the sum consumed equals the parent's, keys travel with the anchor) and only the middle piece gets the swapped source; the plain clip is cut into three pieces of which only the middle one is slowed, the outer pieces are equal to the unlayered clip
- **TEST** t915 (`src/libs/kt-model/tests/morph_slice.rs`), t247 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The cut of a warped clip is neither heard nor seen outside the window
- **WHEN** the projects of t915 are rendered (audio and video) and compared with the same projects without the layer
- **THEN** outside the layer's window the frames are byte-for-byte equal to the unlayered render (CPU reference); the sound before the window is byte-for-byte up to the processing chunk (1024 frames) that holds the seam; after the window it is the same material: after a 10 ms restart of the resampler the best alignment within 16 audio frames leaves a residual under 0.02 (at a constant speed the restart lasts 67 frames and the bytes are then equal; on a speed curve the chunk grid of the new piece starts at its own edge and the material lags ~10 frames, 0.2 ms — the same as a plain split S of such an event); inside the window the swap / the slowdown is present; the former whole-body behaviour fails this test (the red proof)
- **TEST** t916 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

#### Scenario: One rule for a piece's entry point (container, capability `automation`)
- **WHEN** a stretched clip (constant speed, speed curve, reverse) is cut
- **THEN** the piece's entry point is the integral of speed, the seam has no gap, keys travel with the anchor; an unwarped piece is as before
- **TEST** t270 (`src/libs/kt-model/tests/morph_slice.rs`)

## MODIFIED Requirements

### Requirement: "By time" action and reachability
The morpher SHALL act within the window of its block (default `timed = true`); files of the old format are read as "outside time" and render as before. The first question to a target is WHETHER the layer REACHES it by time; only then — cut or patch whole. An empty set of references and a window shorter than the minimal length of an event SHALL mean "did not reach".
Why: the veto "do not cut a warped clip", which stood ahead of the question of reach, handed the warp and the source replacement to the whole two-hour project of the user.

#### Scenario: Format schema and old files
- **WHEN** a file predating the layer schema is read
- **THEN** the morpher receives `timed = false`, the layer raises the version, defaults are not serialized, the round trip is idempotent; a block "outside time" patches the body whole regardless of position
- **TEST** t240 (`src/libs/kt-otio/tests/convert.rs`), t236 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The layer does not reach a warped clip outside the window
- **WHEN** a block with a warp stands 10 seconds away from a stretched clip; a second block covers its middle
- **THEN** the first does not touch the clip at all; the second cuts it at its window, and only the window's piece gets the patch (the former "patches it whole and does not cut it" is inverted with t238 by the owner's decision of 2026-10-28)
- **TEST** t288 (`src/libs/kt-model/tests/morph_slice.rs`)
