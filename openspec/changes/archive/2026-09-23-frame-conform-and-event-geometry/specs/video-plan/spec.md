# Spec Delta

## ADDED Requirements

### Requirement: Layer geometry — placement rectangle, transparent margins, one function
Each video layer of the plan SHALL carry placement geometry: a rectangle in FRACTIONS of the canvas and
the decode size, computed by ONE pure function from the source's view (native × pixel
aspect ratio), the canvas, the conform rule (fit / fill / stretch / as is),
the crop and the zoom. The composite SHALL place the layer into its rectangle; the canvas area outside the
rectangle is alpha 0 (transparent margins), the frame background stays as before. The rule "fit" SHALL
preserve proportions and scale in both directions; "fill" — preserve proportions and crop; "stretch" — pull to the edges; "as is" — place 1:1 pixels centered, with a
source larger than the canvas — past the edge. Placement SHALL run as an affine in the common
geo-program with the edge primitive "transparent", as a pair CPU kernel ↔ WGSL by tolerance class;
an affine with a frame equal to the canvas SHALL give the previous path byte for byte. An unknown source view
SHALL be conformed as "stretch" deterministically.
Why: stretching was accepted by the decoder — outside the single composition code; a black margin on the
top layer erases everything under it (Pitivi #1378); "as is" is the only rule that does not invent pixels.

#### Scenario: Four rules on four sources
- **WHEN** vertical, horizontal, smaller-than-canvas and larger-than-canvas sources (and a source with SAR 2:1) are conformed by each of the four rules
- **THEN** the rectangles equal the analytic ones (fit — margins along the long side, fill — cropping along the short side, stretch — the whole canvas, as is — native centered, past the edge on excess), the decode size is even and not above the ceiling, SAR is accounted for in the view
- **TEST** t712 (`src/libs/kt-fx/tests/conform.rs`)

#### Scenario: Transparent margins on both backends, an identity on matching proportions
- **WHEN** a vertical source is composited into 16:9 on CPU and GPU; a 16:9 source — with an identity affine
- **THEN** the margins are transparent (the lower layer is visible through them), the tolerance class is EXACT at K = 1; the identity affine is byte for byte equal to the previous path
- **TEST** t713 (`src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`), t714b (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Goldens do not move
- **WHEN** the golden exports and "preview == export" are run on fixtures with matching proportions
- **THEN** `GOLDEN_VIDEO`, t11, t67, t184 are green without regeneration; a vertical source gives a frame with margins (a measurement of the geometry of the white rectangle)
- **TEST** t715 (`src/libs/kt-engine/tests/golden_export.rs`)

## MODIFIED Requirements

### Requirement: Decode scale step — a property of the clip and part of the order (order == read)
The decode size of the pre-FX frame SHALL be a property of the clip: the step S ∈ 1..4 =
min(how much the chain asks for by the maximum net zoom over the whole duration of the clip, the native's margin over the conform size, rounded UP, `MAX_INTER_DIM / max(conform size)`);
if the native is unknown → S = 1; the decode is NEVER smaller than the conform size. The conform size — from the
layer's geometry (rule, crop, zoom, canvas); "as is" — the native with the ceiling. The step is
determined by the project, the native and the canvas and MUST NOT depend on the device or budgets.
The decode quality SHALL travel in the order `layers_at`/`wanted_frames` as `(media, pts, Quality)`,
where Quality is the decode size from the geometry: prefetch, composite, export and the warmth bars
ask the same thing. The step of a REEL (the order, one per reel — the maximum over the tree of
instances, from the reel's raster) and the chain input of an INSTANCE (execution) are different numbers;
if they diverge — a sub-composite is fitted before the chain. The chunk budgets of export and the
look-ahead window SHALL be computed by the largest decode quality. A change of the set of sources SHALL
rebuild the player's renderer.
Why: three callers each appended Quality themselves and matched by accident; with a clip-function size
this immediately gives a permanent cache miss and a black frame; the conform rule changes the number of needed pixels, and computing it outside the order is the same permanent miss.

#### Scenario: The step formula
- **WHEN** a clip with a ×2 zoom on a native of 1920×1080 / 2560×1440 / 1280×720 / 640×360 / unknown at a Quality of 1280×720, 4K output, a net-unit chain, zoom 1.01, zoom ×8
- **THEN** the decode is 2560×1440 / 2560×1440 / 1280×720 / 1280×720 / 1280×720; 4K is cut by the ceiling; net zoom ≤ 1 and the quantization cancel the step; the step ceiling is 4
- **TEST** t310 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The step is taken from the maximum zoom over the whole clip
- **WHEN** the zoom is set by a curve 1 → 4 (and the reverse), with a strength stop at zero or on a reversed axis
- **THEN** the step is caught at the curve's maximum from either end, the strength stop does not give the step, the axis reversal does not break it
- **TEST** t311 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The order equals the read, together with the size
- **WHEN** the cache is warmed strictly by `wanted_frames` / warmed "at Quality" as before
- **THEN** the first composite is full, the second — the layer does not find its frame; when the set of (frame, quality) pairs is stretched, the prefetch and the render coincide, the keys strictly increase
- **TEST** t312 (`src/libs/kt-engine/tests/decode_scale.rs`), t247d (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The order equals the read for every conform rule
- **WHEN** a vertical source sits in four events with four rules, the cache is warmed strictly by `wanted_frames`
- **THEN** all four composites are full; the decode size for "as is" is the native, for "fill" — larger than for "fit"; the step is computed from the conform size
- **TEST** t714 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Zoom sharpness on a file with a native margin
- **WHEN** the project is 128×72, the file is 512×288 with stripes of 2 px of native, zoom ×2 / no zoom
- **THEN** the contrast of the finished frame is ≈ 223 out of 255 (on the code before the step — 0.0); without zoom — 0.0: the step is not handed out to everyone indiscriminately
- **TEST** t315 (`src/libs/kt-engine/tests/decode_sharpness.rs`)

#### Scenario: The export chunk budget by decode quality
- **WHEN** step 2 and an export chunk computed by Quality fit the budget, but by the decode quality they do not
- **THEN** the chunk shrinks fourfold, the live render with the step passes with `missing_video_layers == 0`
- **TEST** t314 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A returned file changes the step on the fly
- **WHEN** a source is replaced (the file has returned to disk) without any edit of the model
- **THEN** the player's renderer is rebuilt, the new native changes the step
- **TEST** t316 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: The reel's step is one per reel, the chain input is its own per instance
- **WHEN** two instances of one reel, one of them zooms (including through a published parameter)
- **THEN** the order of the inner leaves is the same for both (the step is the maximum over the instances), the reel's content is not ordered at the step of the zooming input, and an unfitted input is fitted before the chain; the reel's step is part of the post-FX key
- **TEST** t450 (`src/libs/kt-engine/tests/nest_render.rs`), t679 (`src/libs/kt-engine/tests/nest_param.rs`), t631c (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Media substitution in an instance — the order goes to the new address
- **WHEN** a reel instance overrides the media ("logo")
- **THEN** the order goes inward by the new address and the read goes by the same address
- **TEST** t677 (`src/libs/kt-engine/tests/nest_param.rs`)
