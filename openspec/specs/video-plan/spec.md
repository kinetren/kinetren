# Video composition

## Purpose

One frame plan for preview and export: layers, transition and fade weights, levels, the mapping "tick → source frame", the decode scale step and the effect chain are computed by one piece of code, and the backend executor (GPU wgpu by default, CPU is the byte reference and the full fallback) is an execution parameter, not a second path.

## Requirements

### Requirement: The composition code is one; the backend is an execution parameter
The frame plan (layers, order, transition and fade weights, event levels, the mapping "tick →
source frame", the allowed automation curves, frame blending, the decode scale step)
SHALL be built by one piece of code for preview and export. There are two executors — CPU and GPU —
and they MUST only execute the ready plan, without making decisions of their own. Automation
curves SHALL be resolved once when the plan is built: the executors see
ready values. Blending of neighboring frames under stretch SHALL be done in the shared
plan code by the same integer kernel as the crossfade; both executors receive
an already blended pre-FX frame. Public values of a reel instance SHALL be resolved
by the model before the plan.
Why: a second composition path is "the preview shows one thing, the export writes another",
and it is impossible to keep two copies of one arithmetic (the CPU kernel and WGSL) in sync.

#### Scenario: Preview equals export on the CPU path, cold and warm
- **WHEN** a frame with a video-FX chain is composed for preview (without memoization and from the post-FX cache) and taken from the export stream at the same moment
- **THEN** all three frames are byte-for-byte equal
- **TEST** t67 (`src/libs/kt-engine/tests/golden_export.rs`), t26 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: Preview equals export on the GPU path
- **WHEN** the same plan is executed by the GPU engine of the preview (fresh and with a warm plan cache) and by GPU export
- **THEN** the frames are byte-for-byte equal; export is deterministic between runs on one device and lies within tolerance of the CPU reference
- **TEST** t184 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: An animated clip equals a static one at every moment
- **WHEN** the chain parameters are set by curves and the frame is composed at moment t
- **THEN** the result is byte-for-byte equal to the clip's composite with the curve values statically set at that moment; the post-FX cache gives no stale results between different t and after a curve edit
- **TEST** t191 (`src/libs/kt-engine/tests/fx_auto_render.rs`)

#### Scenario: Frame blending lives in the plan, not in the executor
- **WHEN** an event is stretched and the frame mode "blend" is on
- **THEN** the layer's pre-FX frame is a linear mix of two neighboring source frames by the crossfade kernel; at positions exactly on the grid (weight 0) there is no pair and the path stays byte-for-byte as before; prefetch warms both frames of the pair
- **TEST** t250 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: Instance overrides are resolved before the plan
- **WHEN** two instances of one reel are overridden differently (different title), without overrides, an override curve under a trim and a reverse, a template inside a template, a nest with an empty chain
- **THEN** the frames of the instances differ, and the GPU agrees with the CPU for both; without overrides — one sub-renderer and the earlier bytes; the curve lands on the same material frame of the target; the external handle of an inner instance is resolved by the same code one level down; a nest equals the expanded content with the instance's values (the law of E, model — capability `compositions`)
- **TEST** t687 (`src/libs/kt-gpu/tests/gpu_render.rs`), t675 (`src/libs/kt-engine/tests/nest_param.rs`), t676 (`src/libs/kt-engine/tests/nest_param.rs`), t674 (`src/libs/kt-engine/tests/nest_param.rs`), t686 (`src/libs/kt-engine/tests/nest_param.rs`), t688 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: CPU is the byte reference and full fallback; GPU is the default
When a live adapter is present (llvmpipe counts), all video SHALL be computed on the GPU; the CPU path
SHALL remain the full byte reference, including Pan3d (inverse homography in f64) —
export without a GPU is legitimate and only slower, and a refusal "needs GPU" MUST NOT exist.
`TooLarge` from the GPU SHALL move only this plan to the CPU; `DeviceLost`/`Validation`
SHALL "poison" the GPU until the end of the session with a warning exactly once; in export a
GPU error SHALL be an export error — a started export finishes on the backend on which it started,
and the file does not mix backends.
Why: cross-backend bit-exactness is unreachable, so mixing backends within
one file is a visible seam; and a reference with a hole (3D only on the GPU) is not a reference.

#### Scenario: Export without a GPU renders 3D
- **WHEN** a project with Pan3d is exported without the GPU backend
- **THEN** the export does not refuse, the frames actually carry the effect, and with a live adapter the CPU and the GPU agree within the tolerance `GEOM`
- **TEST** t301 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Pan3d — identity at defaults and a check against the CPU on every run
- **WHEN** Pan3d is executed on the GPU with defaults, a shift, and a scale < 1
- **THEN** the defaults give a 1:1 frame, the shift moves the marker by exactly the given px, the scale opens transparent margins, and every frame is checked against the CPU composite within the tolerance `GEOM`
- **TEST** t187 (`src/libs/kt-gpu/tests/pan3d.rs`)

#### Scenario: GPU loss — a warning once, then the CPU silently
- **WHEN** the GPU preview engine fails with `DeviceLost`
- **THEN** the warning is shown exactly once, the preview goes to the CPU until the end of the session, and the backend status is honest
- **TEST** t185 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: GPU↔CPU conformance — only through tolerance classes
The comparison of the GPU with the CPU SHALL go exclusively through `assert_frames_close` with the
`FrameTol` class (EXACT / BILINEAR / THRESHOLD / GEOM); an ad-hoc epsilon MUST NOT
appear. Integer kernels (composite, mirror, noise, tiling) SHALL be
byte-for-byte equal to the CPU; the rules of layer presence and of the `complete` flag on the GPU SHALL
mirror the CPU verbatim; a layer of a foreign size SHALL give `complete = false`, not a
silent skip. The tolerance threshold is set at ×2 from a measurement; a failure "on the edge" is an
investigation, not a tweak. Without an adapter the tests are skipped with a printout,
and `KT_REQUIRE_GPU=1` turns the skip into a failure.
Why: a single tolerance methodology is the only thing that tells "the GPU broke" apart from
"the driver rounded differently"; without a tolerance class every test would invent its own threshold.

#### Scenario: Each effect and chains are within the tolerance of their kind of arithmetic
- **WHEN** each video effect and the characteristic chains (geometry series, tap ops, K×-barriers, Pan3d chains, alpha cases) are executed on both backends
- **THEN** the frames converge within the tolerance class for the kind of arithmetic; `complete` matches
- **TEST** t181 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Composite byte-for-byte
- **WHEN** composite scenes (over, level, solo fade, dissolve, a missing half of a pair, a layer of a foreign size) are executed on the GPU
- **THEN** the frame equals the CPU in `EXACT`, the expected `complete` matches on both, and a foreign size gives `complete = false`
- **TEST** t182 (`src/libs/kt-gpu/tests/parity.rs`), t633 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Conformance cycle over the whole video registry
- **WHEN** for each video effect of the registry a single case and a sandwich "2×2 grid → effect → zoom ×2" are executed
- **THEN** both converge within the tolerance class from the table; a kind without a row in the table fails the test
- **TEST** t329 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Decode scale step on the GPU
- **WHEN** the pre-FX frame arrives twice the size of Quality (`src_scale = 2`)
- **THEN** the conjugation, the leading barriers at 2×, the absorbed pointwise ops, the Legacy mirror at 2× and Pan3d converge with the CPU within the standard tolerances without tweaking
- **TEST** t309 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A reel's step above the chain input — fitting within the tolerance
- **WHEN** the reel's sub-composite is requested at a step above the input of the instance chain
- **THEN** the GPU fits it into the whole frame by a resample and converges with the CPU in `BILINEAR`; at equal steps there is no resample and the bytes stay as before
- **TEST** t632 (`src/libs/kt-gpu/tests/gpu_render.rs`), t631b (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Determinism — byte-for-byte on one device
Video on the CPU path SHALL be byte-for-byte identical between runs and against the committed
reference (GOLDEN_VIDEO is not regenerated; gated linux/x86_64). Video on the GPU path
SHALL be byte-for-byte identical between runs on one device: no atomics,
a fixed order of passes, zeroing load ops, a shader ping-pong composite of two
accumulators, no samplers (bilinear done by hand). Between
machines/drivers only the tolerance to the CPU reference is guaranteed. SIMD kernels SHALL
be byte-for-byte equal to the scalar ones on any CPU; the scalar is the definition of the bytes.
Wall-clock time in offline paths MUST NOT be read.
Why: GPU tests compare exact values only on one device, and cross-backend only by tolerance; any
ordering variable (atomics, fixed-function blending, a driver's sub-texel grid) breaks both.

#### Scenario: Two engines on one adapter and a "dirty" pool
- **WHEN** two independent GPU engines execute one plan, and the texture pool of the first is dirtied by other plans (a geometry series, a K×-barrier)
- **THEN** the frames are byte-for-byte equal; recomputation with a dirty pool and a plan-cache hit give the same bytes
- **TEST** t183 (`src/libs/kt-gpu/tests/parity.rs`), t187b (`src/libs/kt-gpu/tests/pan3d.rs`), t184b — GPU export is deterministic and within tolerance of the CPU (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: AVX2 kernels of composite and bilinear equal the scalar
- **WHEN** alpha_over / dissolve / scale_alpha, division by 255, half-away rounding, bilinear and noise are executed by the AVX2 kernel on non-multiple sizes and extreme values
- **THEN** the result is byte-for-byte equal to the scalar one, including the branches of the scalar (`sa == 0` does not touch dst)
- **TEST** t198 (`src/libs/kt-engine/tests/simd_parity.rs`), t198 (`src/libs/kt-fx/tests/simd_parity.rs`)

### Requirement: The chain plan is built in one place; geometry series in one pass
All decisions about the video effect chain (segmentation into runs and barriers, legacy/fused,
absorption of pointwise ops into tap ops, K× barriers, snapshot points of the layer strength) SHALL
be made by the shared planner in kt-fx; CPU and GPU MUST execute the ready plan. Consecutive
geometric effects SHALL be executed in one pass of inverse mappings
without intermediate rasterizations; a series of length 1 without minification SHALL go through the earlier
kernels byte-for-byte; at K = 1 and step 1 the whole plan is byte-for-byte as before.
Non-local barriers (Blur/Glow/Edges/VHS) SHALL be executed at K× when "compression above +
zoom below", K — the quantum of min(zoom of the tail, compression), the ceiling of the buffer side
`MAX_INTER_DIM = 4096` is a project constant, not a device limit. The layer strength stops
zero the accumulators: mixing is always at 1×. There MUST NOT be any merge switches — neither in the UI nor in env. The formulas of the kernels, `geom.rs`/`plan.rs` and `geom_fused.wgsl`
SHALL be edited in one commit (the effect kernels — capability `video-fx`).
Why: two copies of one chain traversal (CPU and GPU) would drift apart in segmentation,
and segmentation is what decides where a raster is materialized and loses detail.

#### Scenario: The plan of a layer-strength slice equals the piece of the plan of the whole
- **WHEN** a chain is cut into slices along the layer strength stops
- **THEN** the plan of each slice matches the corresponding piece of the plan of the whole chain; the runs/barriers/markers/step budget match on both backends
- **TEST** t306 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Routing and byte guarantees of series
- **WHEN** a series of geometric effects of length 1 without minification / of length > 1 / with a transparent audio FX inside
- **THEN** length 1 goes through the old kernels byte-for-byte, length > 1 goes through the fused pass (grid → mirror equals an exact permutation, mirror∘mirror is an identity), an audio FX does not break the series
- **TEST** t297 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: A net-unit chain returns the detail
- **WHEN** a checkerboard passes "grid 2×2 → zoom ×2" (and the same with a VHS barrier between them)
- **THEN** the contrast comes back whole (not gray from the half-phase of rasterization), and the K× barrier does not kill the detail
- **TEST** t296 (`src/libs/kt-fx/tests/geo_fuse.rs`), t302 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The K rule and the byte anchor K = 1
- **WHEN** chains are planned with an empty / non-zooming / compressing tail, a squeeze cap, "grid + zoom", adjacent barriers and a strength stop
- **THEN** K = 1 in all the first cases, K = 2 for "grid + zoom", adjacent barriers share one K, a strength stop cuts; a K = 1 chain with a barrier gives a hash fixed in advance
- **TEST** t304 (`src/libs/kt-fx/tests/geo_fuse.rs`), t308 (`src/libs/kt-fx/tests/geo_fuse.rs`), t308b (`src/libs/kt-fx/tests/geo_fuse.rs`)

### Requirement: Decode scale step — a property of the clip and part of the order (order == read)
The decode size of the pre-FX frame SHALL be a property of the clip: the scale step S ∈ 1..4 =
min(how much the chain asks by the maximum net zoom over the whole duration of the clip, the margin
of the native over the conform size with rounding UP, `MAX_INTER_DIM / max(conform size)`);
if the native is unknown → S = 1; decoding is NEVER smaller than the conform size. The conform size comes from the
geometry of the layer (rule, crop, zoom, canvas); "as is" — the native with the ceiling. The step
is determined by the project, the native and the canvas, and MUST NOT depend on the device or on budgets.
The decode quality SHALL travel in the order `layers_at`/`wanted_frames` as `(media, pts, Quality)`,
where Quality is the decode size from the geometry: prefetch, composite, export and the coverage bars
ask the same thing. The REEL's step (the order, one per reel — the maximum over the tree of
instances, from the reel's raster) and the INSTANCE chain's input (execution) are different numbers;
if they diverge — the sub-composite is fitted before the chain. The chunk budgets of export and the
lookahead window SHALL be computed by the largest decode quality. A change in the composition of sources SHALL
rebuild the player's renderer.
Why: three callers each glued Quality on themselves and matched by accident; with a size that is a function of the clip,
this immediately gives a permanent cache miss and a black frame; the conform rule changes the number of needed pixels, and computing it outside the order is the same permanent miss.

#### Scenario: The step formula
- **WHEN** a clip with zoom ×2 on natives 1920×1080 / 2560×1440 / 1280×720 / 640×360 / unknown, at Quality 1280×720, 4K output, a net-unit chain, zoom 1.01, zoom ×8
- **THEN** decode 2560×1440 / 2560×1440 / 1280×720 / 1280×720 / 1280×720; 4K is cut by the ceiling; net zoom ≤ 1 and the quantum suppress the step; the ceiling of the step is 4
- **TEST** t310 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The step is taken from the maximum zoom over the whole clip
- **WHEN** the zoom is set by a curve 1 → 4 (and back), with a strength stop at zero or on the reversed axis
- **THEN** the step is caught at the maximum of the curve from either end; a strength stop does not give the step; the reversal of the axis does not break it
- **TEST** t311 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The order equals the read, together with the size
- **WHEN** the cache is warmed strictly by `wanted_frames` / warmed "at Quality" as before
- **THEN** the first composite is full, the second — the layer does not find its frame; when stretching, the sets of pairs (frame, quality) of prefetch and render match, and the keys strictly increase
- **TEST** t312 (`src/libs/kt-engine/tests/decode_scale.rs`), t247d (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The order equals the read for every conform rule
- **WHEN** a vertical source stands in four events with four rules, the cache is warmed strictly by `wanted_frames`
- **THEN** all four composites are full; the decode size of "as is" is the native, of "fill" is larger than that of "fit", the step is counted from the conform size
- **TEST** t714 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Zoom sharpness on a file with a native margin
- **WHEN** the project is 128×72, the file is 512×288 with stripes of 2 native px, zoom ×2 / no zoom
- **THEN** the contrast of the finished frame ≈ 223 of 255 (on the code before the step — 0.0); without zoom — 0.0: the step is not handed out to everyone
- **TEST** t315 (`src/libs/kt-engine/tests/decode_sharpness.rs`)

#### Scenario: The export chunk budget by decode quality
- **WHEN** the step is 2 and the export chunk computed by Quality fits into the budget, while by the decode quality it does not
- **THEN** the chunk is cut to a quarter, the live render with the step passes with `missing_video_layers == 0`
- **TEST** t314 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A returned file changes the step on the fly
- **WHEN** the source is replaced (the file came back to disk) without an edit of the model
- **THEN** the player's renderer is rebuilt, and the new native changes the step
- **TEST** t316 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: The reel's step is one per reel, the chain input is its own per instance
- **WHEN** two instances of one reel, one of which zooms (including through a published parameter)
- **THEN** the order of the inner leaves is the same for both (the step is the maximum over the instances); the content of the reel is ordered not at the step of the zooming input, and an unfitted input is fitted before the chain; the reel's step is part of the post-FX key
- **TEST** t450 (`src/libs/kt-engine/tests/nest_render.rs`), t679 (`src/libs/kt-engine/tests/nest_param.rs`), t631c (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Media substitution in an instance — the order goes to the new address
- **WHEN** an instance of a reel overrides the media ("logo")
- **THEN** the order goes deeper to the new address, and the read goes along the same address
- **TEST** t677 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: The cache is the only truth about coverage
The coverage bars SHALL be `coverage()` of the cache by the DECODE quality of the clip (for a clip cut by a morpher layer — the union of the qualities of the pieces), and not a separate bookkeeping and not a poll by the preview's Quality. Post-FX frames MUST NOT be visible to
the coverage bars, prefetch and pin.
Why: the cache is quality-exact; a clip with a step, warmed properly, would look entirely cold when polled by Quality — a lie in the user's eyes.

#### Scenario: The bars ask about the decode quality
- **WHEN** prefetch has put the frames of a clip with a step strictly by the order
- **THEN** the coverage by the decode quality is non-empty, by the preview's Quality it is empty
- **TEST** t317 (`src/libs/kt-engine/tests/decode_scale.rs`)

### Requirement: Memoization of the post-FX frame and its key
A video post-FX frame SHALL be memoized in the preview as a function of (frame, chain, t)
without history; export MUST NOT memoize post-FX (each t is composed once,
full-size frames would evict the pre-frames of neighboring tracks). The key SHALL include the
hash of the chain with its curves, the geometry of the clip, the resolved camera (it is selected
per frame and is not visible to a static hash), the decode step of the input, and the step of the reel. On the
GPU, memoization is a separate cache of the whole plan keyed by the plan key, which SHALL include the
parameters of the transition kind, the camera, the step, the reel's step and the blend flag;
the post-FX cache of kt-cache is a mechanism of the CPU path only.
Why: any parameter that affects the bytes and is not in the key is a stale value, which shows up as "did not apply".

#### Scenario: A repeat from the cache, an edit — a recomputation
- **WHEN** the frame of an FX clip is composed again / after a change of the chain parameters or a shift of the clip
- **THEN** the repeat is byte-identical and is read from the cache without applying the chain; the change alters the key, and no stale value is read
- **TEST** t66 (`src/libs/kt-engine/tests/postfx_cache.rs`)

#### Scenario: The export compose does not memoize
- **WHEN** the same frame is composed by the export path and by the preview path
- **THEN** the export path does not inflate the post-FX cache with frames, the preview path memoizes, and the bytes are equal
- **TEST** t66c (`src/libs/kt-engine/tests/postfx_cache.rs`)

#### Scenario: The camera is in the key
- **WHEN** the camera is edited or moved while the clip's fx hash with Pan3d stays the same
- **THEN** the frame is recomputed, and no stale value is read
- **TEST** t66d (`src/libs/kt-engine/tests/postfx_cache.rs`)

#### Scenario: The step is in the key
- **WHEN** one clip at one Quality is composed from input 1× and from input 2×
- **THEN** both frames lie in the cache at the same time, and repeats return each its own bytes
- **TEST** t313 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Transition parameters in the GPU plan key
- **WHEN** each transition kind is executed at several phases, both axes of direction, and both states of the flags
- **THEN** the GPU converges with the CPU within tolerance (volumetric ones — `GEOM`); a transition frame of another kind is not returned from the plan cache
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: The mapping "tick → source frame" is one function for all consumers
The choice of the source frame (reverse, stretch by the integral of the signed speed,
blending of pairs) SHALL be computed by one function, which feeds `layers_at`,
`wanted_frames`, prefetch, preview and export. Reverse SHALL give the frame of the mirror
pts within the piece of the source; reverse + stretch is "warp, then mirror", the mirror axis
is the span (min P, max P). A clip without a warp SHALL go through the earlier path byte-for-byte.
Why: if the order diverges from the read — a permanent cache miss and a black frame; this is the
video analogue of the rule "a single source of truth for prefetch and render".

#### Scenario: Reverse — the mirror pts
- **WHEN** an event that is reversed is composed on frames i
- **THEN** the pts of the source equals the pts of the forward event on the mirror frame
- **TEST** t24 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: Video follows speed, curve and reverse
- **WHEN** an event with constant / curved speed / reverse on top of a stretch
- **THEN** the pts track follows the integral of the speed, and reverse with stretch is composed
- **TEST** t247 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: Signed speed — motion backwards and a mirror over the span
- **WHEN** a speed curve 1 → −1 / constant −0.5 with reverse enabled
- **THEN** the material rises to the middle and returns to the start, the second half strictly decreases; reverse over negative speed goes forward monotonically
- **TEST** t604a (`src/libs/kt-engine/tests/video_warp.rs`), t604g (`src/libs/kt-engine/tests/video_warp.rs`)

### Requirement: Layers, transition and fade weights, event level — in the plan
A pair of clips in a transition window SHALL be blended into one layer with the integer weight k/4096
before alpha_over; the missing half of the pair is the present one at full weight with
`complete = false`; a solo fade is the same kernel with an empty second side, the layer
is multiplied by the alpha, `complete` is not reset (there is no pair by the model). The level
of the event SHALL be multiplied by the alpha of the layer where the fades are; for a pair the levels are blended
with the same weight as the picture; 1.0 is an identity. The inversion of a transition SHALL be executed
by the plan (swap of sides + reverse progress), without touching the kernels and keys; an overlaid
transition is taken from the transitions of the reel, not of the root. The transition kind is turned
into flat parameters exactly once; the CPU kernel and WGSL read the same numbers (the kernels — capability
`transitions`).
Why: weights and levels computed in the executor would diverge between backends and
between preview and export.

#### Scenario: Dissolve of a pair, the missing half, handles, a chain on the pair
- **WHEN** a crossfade between two clips is composed in the window, with one half in the cache, with handles beyond the edges of the piece, and with an FX chain on the pair
- **THEN** the pair is blended linearly and deterministically; the missing half gives full weight and `complete = false`; the layers include the handles; the chain on the pair survives the dissolve
- **TEST** t137 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: A solo fade scales the alpha and stays full
- **WHEN** a fade at the edge of the event (in/out)
- **THEN** the alpha of the layer follows the curve, `complete` remains true, at the outgoing edge the frame darkens
- **TEST** t156 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: The level of the event scales the layer
- **WHEN** the event's level is set to < 1
- **THEN** the layer is multiplied by the level where the fades are; 1.0 — an identity byte-for-byte
- **TEST** t174 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: The transition kind reaches the picture
- **WHEN** wipe / shutters / push / dip / new 2D families / volumetric kinds are composed in the middle of the window
- **THEN** the zones of the frame match the analytical expectations of the kind (a sharp boundary, symmetry of the edges, exchange of halves, fading to color, transparency behind the panels), the crossfade is a uniform mix
- **TEST** t225 (`src/libs/kt-engine/tests/transitions_video.rs`), t467 (`src/libs/kt-engine/tests/transitions_video.rs`), t468 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: The inversion is executed by the plan
- **WHEN** a transition is marked as inverted
- **THEN** the crossfade is an identity, the directional kind matches the opposite direction, the solo fade is cut from the other side
- **TEST** t461 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: An overlaid transition reaches the picture
- **WHEN** two clips are overlaid without a transition / with a transition
- **THEN** without a transition — a hard cut (the later one on top is opaque); with a transition the pair collapses into a dissolve and the kind is visible in the frame
- **TEST** t464 (`src/libs/kt-engine/tests/transitions_video.rs`)

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
- **THEN** the transition is reattached to the piece with the joint and the frame equals the one cut by hand; the result is byte-for-byte as before; a warped clip is not cut at all
- **TEST** t237 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t238 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

### Requirement: The camera is resolved by one rule per programme
The 3D camera for Pan3d SHALL be resolved by one function: pinned by id → active by time → the default "50 mm" (an identity for Pan3d with defaults); a camera pinned to a nonexistent id falls back to the active one; a chain without a Pan3d camera does not get one.
`world_scale` = the size of the target / the size of the project — the camera world is frozen by the format in the
world pixels of the project. The semantics of the world (X to the right, Y down, Z into depth, Rz·Rx·Ry,
auto-distance) MUST NOT change.
Why: the second consumer (the picker's thumbnails) physically could not call the rule from a
private function and would have rewritten it; they would have diverged silently — the variant in the picker
would look different from the one after a click.

#### Scenario: Pinned → active → default, the target scale
- **WHEN** the camera is resolved for a chain with/without Pan3d, with a pinned, deleted and missing id, for a target of 320 px in a 1920 px project
- **THEN** the order of priorities is kept, the default equals `CameraParams::default()`, `world_scale` = 1/6, a zero project size does not divide by zero
- **TEST** t545 (`src/libs/kt-engine/tests/cam_resolve.rs`)

### Requirement: Layer geometry — placement rectangle, transparent margins, one function
Each video layer of the plan SHALL carry placement geometry: a rectangle in FRACTIONS of the canvas and the decode size, computed by ONE pure function from the kind of source (native × pixel aspect), the canvas, the conform rule (fit / fill / stretch / as is), the crop and the zoom. The composite SHALL put the layer into its rectangle; the canvas area outside the rectangle is alpha 0 (transparent margins), the frame background is the earlier one. The "fit" rule SHALL keep the proportions and scale in both directions; "fill" — keep the proportions and crop; "stretch" — pull to the edges; "as is" — put 1:1 pixel in the center, and if the source is larger than the canvas — past the edge. The placement SHALL be executed as an affine in the shared geometry program with the edge primitive "transparent", as a CPU kernel ↔ WGSL pair by tolerance classes; an affine with a frame equal to the canvas SHALL give the earlier path byte-for-byte. An unknown kind of source SHALL be conformed as "stretch" deterministically.
THIS SAME function SHALL hand the placement rectangle OUTWARD — to the interface that draws the
frame over the picture: the rectangle in fractions of the canvas, the rotation angle and the anchor point in the same
coordinates the composite sees. The inverse computation "canvas point → node parameter value" SHALL be its
pair in the same place: moving the point on the canvas gives the position, pulling a corner gives the zoom,
dragging the rotation handle gives the angle, moving the anchor gives the anchor TOGETHER with a compensation of the position, under which
the placement rectangle does not change (at any rotation). There MUST NOT be a second arithmetic of placement
in the program.
Why: stretching was accepted by the decoder — outside the single composition code; a black margin on
the top layer erases everything under it (Pitivi #1378); "as is" is the only rule that does not invent pixels. R.64: an on-screen gesture over the picture must compute exactly the frame that will go to the file, otherwise the control shows one thing and the export gives another.

#### Scenario: Four rules on four sources
- **WHEN** vertical, horizontal, smaller-than-canvas and larger-than-canvas sources (and a source with SAR 2:1) are conformed by each of the four rules
- **THEN** the rectangles equal the analytical ones (fit — margins along the long side, fill — cropping along the short side, stretch — the whole canvas, as is — the native centered, past the edge on excess); the decode size is even and not above the ceiling; the SAR is taken into account in the shape
- **TEST** t712 (`src/libs/kt-fx/tests/conform.rs`)

#### Scenario: Transparent margins on both backends, an identity on matching proportions
- **WHEN** a vertical source is composed in 16:9 on the CPU and the GPU; a 16:9 source — with an identity affine
- **THEN** the margins are transparent (the lower layer is visible through them), the tolerance class is EXACT at K = 1; the identity affine is byte-for-byte equal to the earlier path
- **TEST** t713 (`src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`), t714b (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Goldens do not move
- **WHEN** the golden exports and "preview == export" are run on fixtures with matching proportions
- **THEN** `GOLDEN_VIDEO`, t11, t67, t184 are green without regeneration; a vertical source gives a frame with margins (a measurement of the geometry of a white rectangle)
- **TEST** t715 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: The rectangle outward and the inverse computation
- **WHEN** the event has a crop, zoom, position, rotation and anchor set; the interface asks for the placement rectangle; then asks to recompute the shift of a canvas point into a position, a corner pull into zoom, and an anchor move
- **THEN** the rectangle equals the one by which the composite places the layer (checked against the frame); the inverse computation returns values that give exactly the requested shift; the anchor move with compensation leaves the rectangle unchanged at rotation 0° and 30°
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`)

### Requirement: Track as an implicit reel in the plan; blend modes as a pair; video mute/solo
A video track with a non-empty chain, a level ≠ 1 or a blend mode ≠ Normal SHALL
enter the plan as one nested layer: the composite of its events (transitions, overlaps, fades,
event levels — inside) by the same code as a nested reel (the raster — the parent's canvas, the time
— the parent's, the step — the maximum over its events), then the track's chain, then alpha ×
level and the blend down with the chosen mode. A track without these properties SHALL
be flattened into layers as before — byte-for-byte and at the same cost. A muted (mute / not
in solo) video track MUST NOT be composited, but its prefetch order SHALL be kept. The blend
mode SHALL be computed as `out = lerp(below, blend(below, above), α_above × level)`;
Normal — the earlier alpha_over byte-for-byte; the set: Normal, Multiply, Screen, Add, Subtract,
Darken, Lighten, Overlay, Difference; the CPU kernel and WGSL are edited as a pair by the tolerance class
of transitions. A hole on a track is the transparency of its composite, not an operand. The memo of the track's composite
SHALL be keyed by (track, track revision, tick) by the same mechanism as the reel's; the morpher
remains per-clip and stands BEFORE the track stage.
Why: a chain appended to every event gives a different picture at transitions and overlaps and cannot do a vignette "over everything" (F-MPH-005/007); a mode on a track whose operand is emptiness would ink the lower layers in Vegas (F-CMP-003); a mute that loses the order would give a black frame on unmuting.

#### Scenario: Without a condition — the earlier bytes and cost; with a chain — the track's composite
- **WHEN** a project without track properties; then a vignette on a track with a transition between two events; a second composite of the same tick; export
- **THEN** t11/t67/t184/`GOLDEN_VIDEO` are byte-for-byte and the number of layers is as before; the vignette covers the transition entirely and is not equal to a vignette appended to every event; the second composite hits the track's memo; preview == export
- **TEST** t736 (`src/libs/kt-engine/tests/track_video.rs`)

#### Scenario: Blend modes against the formula, as a pair
- **WHEN** each mode is applied to a pair of tracks with known colors; the track level is 0.5; on the track with Multiply — a hole; the same plans on the GPU
- **THEN** the result equals the analytical formula; Normal is byte-for-byte as before; 0.5 — half the way to the result of the mode, not the mode over a semi-transparent layer; in the hole the lower layers are visible without darkening; GPU conformance is within the tolerance class of transitions
- **TEST** t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Video mute and solo
- **WHEN** a track is muted; then two of three are soloed; then mute is removed
- **THEN** the frame is without the muted track, `wanted_frames` contains its frames; in solo exactly two are composited; after removing mute the frame is full with no cache miss
- **TEST** t735 (`src/libs/kt-engine/tests/track_video.rs`)
