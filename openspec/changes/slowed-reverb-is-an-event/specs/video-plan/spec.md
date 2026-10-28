## MODIFIED Requirements

### Requirement: The morpher layer in the plan — strength as a mix slot; the cut is not visible
The layer strength SHALL enter the plan as a snapshot point / blend slot of the processed picture
with the unprocessed one: strength 1.0 — the earlier frame byte-for-byte (no slot), 0.0 — the frame without
the layer's contribution but with the event's own effects, an intermediate value — a linear mix;
the block's fades use the same strength curve. The chain is passed to the executor in whole slices
between the strength stops. A transition on a clip cut by a layer SHALL remain live (the end
of the transition is found by "the same clip" + a check of the boundary), and a cut that changes nothing
for this kind SHALL give the earlier result byte-for-byte (the cut model — capability
`morpher`).
Why: the id of the original after a cut carries only piece №0, and a comparison by id silently
lost the transition; a strength that cut a chain at one effect broke the fusion of geometry series.

#### Scenario: The layer strength blends only the layer's contribution
- **WHEN** an event with its own effect is covered by a layer with strength 1.0 / 0.0 / 0.5 and a block with fades at the edges
- **THEN** 1.0 — the earlier frame byte-for-byte, 0.0 — the frame without the layer with its own effect, 0.5 — exactly a linear mix; at the edge of the block the layer has no effect, in the middle — fully; in the sound the same rule
- **TEST** t267 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t267b (`src/libs/kt-engine/tests/morph_slice_render.rs`), t268 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

#### Scenario: A transition on a cut clip is alive, the cut is neither heard nor seen
- **WHEN** a layer cuts the tail of a clip together with the crossfade window / a patch does nothing to this kind of clip / a clip with a time warp
- **THEN** the transition is reattached to the piece with the joint and the frame equals the one cut by hand; the result is byte-for-byte as before; a warped clip is cut at the integral of its speed and, outside the layer's window, its frames are byte-for-byte as without the layer and its sound is the same material within a resampler restart at the seam (t916 states the measure)
- **TEST** t237 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t238 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t916 (`src/libs/kt-engine/tests/morph_slice_render.rs`)
