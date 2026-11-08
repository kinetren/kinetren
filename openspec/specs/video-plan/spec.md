# Video composition

## Purpose

One frame plan for preview and export: layers, transition and fade weights, levels, the mapping "tick → source frame", the decode scale step and the effect chain are computed by one piece of code, and one executor, the GPU (wgpu, `dyn VideoGpuExec`), runs it. With no device there is no picture.

## Requirements

### Requirement: The composition code is one; one executor runs the plan
The frame plan (layers, order, transition and fade weights, event levels, the mapping "tick →
source frame", the allowed automation curves, frame blending, the decode scale step)
SHALL be built by one piece of code for preview and export. There is one executor, the GPU
(`dyn VideoGpuExec`, implemented in kt-gpu), and it MUST only execute the ready plan, without
making decisions of its own. Automation curves SHALL be resolved once when the plan is built: the
executor sees ready values. Blending of neighboring frames under stretch SHALL be done in the
shared plan code by an integer kernel (`dissolve_into`); the executor receives an already blended
pre-FX frame. Public values of a reel instance SHALL be resolved by the model before the plan.
Why: a second composition path is "the preview shows one thing, the export writes another". The
CPU composite was that second path until the owner had it deleted (compute-kernels-render,
2026-11-04).

#### Scenario: Preview equals export, cold and warm
- **WHEN** a frame with a video-FX chain is composed for preview (fresh and from the readback memo) and taken from the export stream at the same moment
- **THEN** all three frames are byte-for-byte equal
- **TEST** t67 (`src/libs/kt-engine/tests/golden_export.rs`), t26 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: A fresh engine and a warm one give the export's frame
- **WHEN** the same plan is executed by a fresh engine, by the engine that exported it (a warm plan memo) and by export
- **THEN** the frames are byte-for-byte equal, and export is deterministic between runs on one device
- **TEST** t184, t184b (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: An animated clip equals a static one at every moment
- **WHEN** the chain parameters are set by curves and the frame is composed at moment t
- **THEN** the result is byte-for-byte equal to the clip's composite with the curve values statically set at that moment; the memo gives no stale results between different t and after a curve edit
- **TEST** t191 (`src/libs/kt-engine/tests/fx_auto_render.rs`)

#### Scenario: Frame blending lives in the plan, not in the executor
- **WHEN** an event is stretched and the frame mode "blend" is on
- **THEN** the layer's pre-FX frame is a linear mix of two neighboring source frames by the plan's integer kernel; at positions exactly on the grid (weight 0) there is no pair and the path stays byte-for-byte as before; prefetch warms both frames of the pair
- **TEST** t250 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: Instance overrides are resolved before the plan
- **WHEN** two instances of one reel are overridden differently (different title), without overrides, an override curve under a trim and a reverse, a template inside a template, a nest with an empty chain
- **THEN** the frames of the instances differ, and each is within tolerance of its stored frame; without overrides — one sub-renderer and the earlier bytes; the curve lands on the same material frame of the target; the external handle of an inner instance is resolved by the same code one level down; a nest equals the expanded content with the instance's values (the law of E, model — capability `compositions`)
- **TEST** t687 (`src/libs/kt-gpu/tests/gpu_render.rs`), t675 (`src/libs/kt-engine/tests/nest_param.rs`), t676 (`src/libs/kt-engine/tests/nest_param.rs`), t674 (`src/libs/kt-engine/tests/nest_param.rs`), t686 (`src/libs/kt-engine/tests/nest_param.rs`), t688 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: The GPU is the one executor; with no device there is no picture
All video SHALL be computed on a live adapter; llvmpipe counts. With no Vulkan, DX12 or Metal device
an export SHALL fail with `ExportError::NoDevice` ("no video device"), and the preview SHALL have no
picture and say so (`VideoBackendStatus::Missing`, the performance line "FX: no device"). `TooLarge`
on a preview plan SHALL give no picture for that plan, and in an export it SHALL be an error.
`DeviceLost`/`Validation` SHALL "poison" the device until the end of the session: no picture, a
warning exactly once. In export a GPU error SHALL be an export error.
Why: the owner, told that the render on a machine with no card would cost more, deleted the CPU
composite ("удаляй", 2026-11-04). A silent substitute for the picture would look like a healthy
program.

#### Scenario: An export renders 3D, and with no device it refuses
- **WHEN** a project with Pan3d is exported on a device; then a project is exported with no device
- **THEN** the frames actually carry the effect; the second export fails with "no video device"
- **TEST** t301 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Pan3d — identity at defaults and a check against the stored frames on every run
- **WHEN** Pan3d is executed with defaults, a shift, and a scale < 1
- **THEN** the defaults give a 1:1 frame, the shift moves the marker by exactly the given px, the scale opens transparent margins, and every frame is checked against its stored frame within the tolerance `GEOM`
- **TEST** t187 (`src/libs/kt-gpu/tests/pan3d.rs`)

#### Scenario: GPU loss — a warning once, then no picture
- **WHEN** the GPU preview engine fails with `DeviceLost`
- **THEN** the warning is shown exactly once, the preview has no picture until the end of the session, and the backend status is honest
- **TEST** t185 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The GPU is held to stored frames, only through tolerance classes
The tests SHALL hold the GPU to the stored frames of the last CPU composite in
`src/libs/kt-gpu/references/`, read through `kt_gpu::reference`, exclusively through
`assert_frames_close` with the `FrameTol` class (EXACT / BILINEAR / THRESHOLD / GEOM / SIGNAL); an
ad-hoc epsilon MUST NOT appear. Integer kernels (composite, mirror, noise, tiling) SHALL be
byte-for-byte equal to their stored frames; a layer of a foreign size SHALL give `complete = false`,
not a silent skip. The tolerance threshold is set at ×2 from a measurement; a failure "on the edge"
is an investigation, not a tweak. Without an adapter the tests are skipped with a printout, and
`KT_REQUIRE_GPU=1` turns the skip into a failure.
Why: a single tolerance methodology is the only thing that tells "the GPU broke" apart from
"the driver rounded differently"; without a tolerance class every test would invent its own threshold.

#### Scenario: Each effect and chains are within the tolerance of their kind of arithmetic
- **WHEN** each video effect and the characteristic chains (geometry series, tap ops, K×-barriers, Pan3d chains, alpha cases) are executed
- **THEN** the frames are within the tolerance class for the kind of arithmetic of their stored frames; `complete` matches
- **TEST** t181 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Composite byte-for-byte
- **WHEN** composite scenes (over, level, solo fade, dissolve, a missing half of a pair, a layer of a foreign size) are executed on the GPU
- **THEN** the frame equals its stored frame in `EXACT`, the expected `complete` matches, and a foreign size gives `complete = false`
- **TEST** t182 (`src/libs/kt-gpu/tests/parity.rs`), t633 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A cycle over the whole video registry
- **WHEN** for each video effect of the registry a single case and a sandwich "2×2 grid → effect → zoom ×2" are executed
- **THEN** both are within the tolerance class from the table of their stored frames; a kind without a row in the table fails the test
- **TEST** t329 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Decode scale step on the GPU
- **WHEN** the pre-FX frame arrives twice the size of Quality (`src_scale = 2`)
- **THEN** the conjugation, the leading barriers at 2×, the absorbed pointwise ops, the Legacy mirror at 2× and Pan3d stay within the standard tolerances of their stored frames without tweaking
- **TEST** t309 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A reel's step above the chain input — fitting within the tolerance
- **WHEN** the reel's sub-composite is requested at a step above the input of the instance chain
- **THEN** the GPU fits it into the whole frame by a resample and stays within `BILINEAR` of the stored frame; at equal steps there is no resample and the bytes stay as before
- **TEST** t632 (`src/libs/kt-gpu/tests/gpu_render.rs`), t631b (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Determinism — byte-for-byte on one device
Video SHALL be byte-for-byte identical between runs on one device: no atomics, a fixed order of
passes, zeroing load ops, a shader ping-pong composite of two accumulators, no samplers (bilinear
done by hand). Between machines and drivers it SHALL stay within the tolerances of the stored
frames. A committed byte hash of video (GOLDEN_VIDEO, GOLDEN_TITLE_VIDEO) SHALL hold on a software
adapter only (`hash_holds`); a card is held to the stored frames. Audio is byte for byte always
(capability `audio-render`). Wall-clock time in offline paths MUST NOT be read.
Why: exact values compare only on one device, and across devices only by tolerance; any ordering
variable (atomics, fixed-function blending, a driver's sub-texel grid) breaks both.

#### Scenario: Two engines on one adapter and a "dirty" pool
- **WHEN** two independent GPU engines execute one plan, and the texture pool of the first is dirtied by other plans (a geometry series, a K×-barrier)
- **THEN** the frames are byte-for-byte equal; recomputation with a dirty pool and a plan-cache hit give the same bytes
- **TEST** t183 (`src/libs/kt-gpu/tests/parity.rs`), t187b (`src/libs/kt-gpu/tests/pan3d.rs`), t184b — GPU export is deterministic on the device (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: The chain plan is built in one place; geometry series in one pass
All decisions about the video effect chain (segmentation into runs and barriers, legacy/fused,
absorption of pointwise ops into tap ops, K× barriers, snapshot points of the layer strength) SHALL
be made by the shared planner in kt-fx; the executor MUST execute the ready plan. Consecutive
geometric effects SHALL be executed in one pass of inverse mappings
without intermediate rasterizations; a series of length 1 without minification SHALL go through the earlier
kernels byte-for-byte; at K = 1 and step 1 the whole plan is byte-for-byte as before.
Non-local barriers (Blur/Glow/Edges/VHS) SHALL be executed at K× when "compression above +
zoom below", K — the quantum of min(zoom of the tail, compression), the ceiling of the buffer side
`MAX_INTER_DIM = 4096` is a project constant, not a device limit. The layer strength stops
zero the accumulators: mixing is always at 1×. There MUST NOT be any merge switches — neither in the UI nor in env. A change of the program `geom.rs`/`plan.rs` build and of `geom_fused.wgsl`,
which reads it, SHALL be edited in one commit (the effect kernels — capability `video-fx`).
Why: the chain traversal has one home because segmentation is what decides where a raster is
materialized and loses detail.

#### Scenario: The plan of a layer-strength slice equals the piece of the plan of the whole
- **WHEN** a chain is cut into slices along the layer strength stops
- **THEN** the plan of each slice matches the corresponding piece of the plan of the whole chain; the runs/barriers/markers/step budget match
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
- **THEN** the order of the inner leaves is the same for both (the step is the maximum over the instances); the content of the reel is ordered not at the step of the zooming input, and an unfitted input is fitted before the chain; the reel's step is part of the plan key
- **TEST** t450 (`src/libs/kt-engine/tests/nest_render.rs`), t679 (`src/libs/kt-engine/tests/nest_param.rs`), t631c (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Media substitution in an instance — the order goes to the new address
- **WHEN** an instance of a reel overrides the media ("logo")
- **THEN** the order goes deeper to the new address, and the read goes along the same address
- **TEST** t677 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: The cache is the only truth about coverage
The coverage bars SHALL be `coverage()` of the cache by the DECODE quality of the clip (for a clip cut by a morpher layer — the union of the qualities of the pieces), and not a separate bookkeeping and not a poll by the preview's Quality. Memoized composites MUST NOT be visible to
prefetch and pin. Over the decoded ranges the bars SHALL draw what plays without work: the moments whose presented composite the executor still holds and the current plan still asks (`Player::final_video_runs`), and the frames the audio render thread's final mix holds (`Player::final_audio_runs`).
Why: the cache is quality-exact; a clip with a step, warmed properly, would look entirely cold when polled by Quality — a lie in the user's eyes. A decoded range still needs its composite and its effects: the bars promised a free play that was not (kkt-26.11.07b, "не рабочий кэш").

#### Scenario: The bars ask about the decode quality
- **WHEN** prefetch has put the frames of a clip with a step strictly by the order
- **THEN** the coverage by the decode quality is non-empty, by the preview's Quality it is empty
- **TEST** t317 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The bars show what plays without work
- **WHEN** a range is played once with a device and an edit then changes one event in it
- **THEN** the bars' final layer holds the played moments and frames, and after the edit the edited event's moments leave it while the others stay
- **TEST** t1936 (`src/libs/kt-engine/tests/final_runs.rs`)

#### Scenario: A hover over kept ranges costs no layout
- **WHEN** the strips carry a kept layer over the events and the pointer passes over them for 28 frames
- **THEN** the events are laid out once, and the layer stays drawn
- **PATH** play a range once → the strips over it turn bright → move the mouse over its events on a pause → nothing on the timeline is laid out again, the bright layer stays
- **TEST** t1938 (`src/apps/kt-app/src/idle_tests.rs`)

#### Scenario: A covered loop plays from what is kept, live
- **PATH** open the heavy showcase with a half-second loop over the composition → press Space → the first lap composes, then the strips over the loop turn bright → the picture plays at the project's rate, no frame is composed, no burst runs the effects, nothing is decoded, no underrun → press Space → nothing runs
- **TEST** live scenario `src/scripts/live/cached_range.py`

### Requirement: Memoization of the composite and its key
A composed frame SHALL be memoized in the preview as a function of the whole plan, keyed by the plan key: on the readback path in the executor's readback memo, on the display path in the executor's present memo (`PresentMemo`), where presenting a held key runs only the display pass (`GpuStages.replayed`, the work row `composite_replays`). Only complete frames are kept; the present memo is bounded (512 MB), least recently shown first. The key SHALL include the hash of the chain with its
curves, the geometry of the clip, the resolved camera (it is selected per frame and is not visible to
a static hash), the masks of the frame, the decode step of the input, the parameters of the
transition kind, the step of the reel and the blend flag. Export MUST NOT memoize (each t is
composed once).
Why: any parameter that affects the bytes and is not in the key is a stale value, which shows up as "did not apply". preview-throughput D2 moved the preview to the display path, which had no memo: a covered range was composed again on every lap — 16 V-fps on the owner's project (kkt-26.11.07b, line 3).

#### Scenario: The step is in the key
- **WHEN** one clip at one Quality is composed from input 1× and from input 2×
- **THEN** the two frames differ, and repeats return each its own bytes
- **TEST** t313 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Transition parameters in the GPU plan key
- **WHEN** each transition kind is executed at several phases, both axes of direction, and both states of the flags
- **THEN** the frames are within tolerance of their stored frames (volumetric ones — `GEOM`); a transition frame of another kind is not returned from the plan cache
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A presented composite is kept
- **WHEN** ten moments are presented twice, then one event's level changes and they are presented again
- **THEN** the second presents are kept composites with nothing composed and the same picture; after the edit exactly the moments of the edited event compose again
- **TEST** t1934 (`src/libs/kt-gpu/tests/preview_display.rs`)

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
into flat parameters exactly once; the WGSL kernels read those numbers (the kernels — capability
`transitions`).
Why: weights and levels computed in the executor would escape the plan and could diverge
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
- **THEN** the transition is reattached to the piece with the joint and the frame equals the one cut by hand; the result is byte-for-byte as before; a warped clip is cut at the integral of its speed and, outside the layer's window, its frames are byte-for-byte as without the layer and its sound is the same material within a resampler restart at the seam (t916 states the measure)
- **TEST** t237 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t238 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t916 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

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
Each video layer of the plan SHALL carry placement geometry: a rectangle in FRACTIONS of the canvas and the decode size, computed by ONE pure function from the kind of source (native × pixel aspect), the canvas, the conform rule (fit / fill / stretch / as is), the crop and the zoom. The composite SHALL put the layer into its rectangle; the canvas area outside the rectangle is alpha 0 (transparent margins), the frame background is the earlier one. The "fit" rule SHALL keep the proportions and scale in both directions; "fill" — keep the proportions and crop; "stretch" — pull to the edges; "as is" — put 1:1 pixel in the center, and if the source is larger than the canvas — past the edge. The placement SHALL be executed as an affine in the shared geometry program with the edge primitive "transparent"; an affine with a frame equal to the canvas SHALL give the earlier path byte-for-byte. An unknown kind of source SHALL be conformed as "stretch" deterministically.
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

#### Scenario: Transparent margins, an identity on matching proportions
- **WHEN** a vertical source is composed in 16:9; a 16:9 source — with an identity affine
- **THEN** the margins are transparent (the lower layer is visible through them), the tolerance class against the stored frame is EXACT at K = 1; the identity affine is byte-for-byte equal to the earlier path
- **TEST** t713 (`src/libs/kt-fx/tests/video_fx.rs`), t714b (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Goldens do not move
- **WHEN** the golden exports and "preview == export" are run on fixtures with matching proportions
- **THEN** `GOLDEN_VIDEO`, t11, t67, t184 are green without regeneration; a vertical source gives a frame with margins (a measurement of the geometry of a white rectangle)
- **TEST** t715 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: The rectangle outward and the inverse computation
- **WHEN** the event has a crop, zoom, position, rotation and anchor set; the interface asks for the placement rectangle; then asks to recompute the shift of a canvas point into a position, a corner pull into zoom, and an anchor move
- **THEN** the rectangle equals the one by which the composite places the layer (checked against the frame); the inverse computation returns values that give exactly the requested shift; the anchor move with compensation leaves the rectangle unchanged at rotation 0° and 30°
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`)

### Requirement: Track as an implicit reel in the plan; blend modes; video mute/solo
A video track with a non-empty chain, a level ≠ 1 or a blend mode ≠ Normal SHALL
enter the plan as one nested layer: the composite of its events (transitions, overlaps, fades,
event levels — inside) by the same code as a nested reel (the raster — the parent's canvas, the time
— the parent's, the step — the maximum over its events), then the track's chain, then alpha ×
level and the blend down with the chosen mode. A track without these properties SHALL
be flattened into layers as before — byte-for-byte and at the same cost. A muted (mute / not
in solo) video track MUST NOT be composited, but its prefetch order SHALL be kept. The blend
mode SHALL be computed as `out = lerp(below, blend(below, above), α_above × level)`;
Normal — the earlier alpha_over byte-for-byte; the set: Normal, Multiply, Screen, Add, Subtract,
Darken, Lighten, Overlay, Difference; the WGSL kernel is held to the tolerance class
of transitions. A hole on a track is the transparency of its composite, not an operand. The memo of the track's composite
SHALL be keyed by (track, track revision, tick) by the same mechanism as the reel's; the morpher
remains per-clip and stands BEFORE the track stage.
Why: a chain appended to every event gives a different picture at transitions and overlaps and cannot do a vignette "over everything" (F-MPH-005/007); a mode on a track whose operand is emptiness would ink the lower layers in Vegas (F-CMP-003); a mute that loses the order would give a black frame on unmuting.

#### Scenario: Without a condition — the earlier bytes and cost; with a chain — the track's composite
- **WHEN** a project without track properties; then a vignette on a track with a transition between two events; a second composite of the same tick; export
- **THEN** t11/t67/t184/`GOLDEN_VIDEO` are byte-for-byte and the number of layers is as before; the vignette covers the transition entirely and is not equal to a vignette appended to every event; the second composite hits the track's memo; preview == export
- **TEST** t736 (`src/libs/kt-engine/tests/track_video.rs`)

#### Scenario: Blend modes against the formula
- **WHEN** each mode is applied to a pair of tracks with known colors; the track level is 0.5; on the track with Multiply — a hole; the same plans on the GPU
- **THEN** the result equals the analytical formula; Normal is byte-for-byte as before; 0.5 — half the way to the result of the mode, not the mode over a semi-transparent layer; in the hole the lower layers are visible without darkening; the frames are within the tolerance class of transitions of their stored frames
- **TEST** t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Video mute and solo
- **WHEN** a track is muted; then two of three are soloed; then mute is removed
- **THEN** the frame is without the muted track, `wanted_frames` contains its frames; in solo exactly two are composited; after removing mute the frame is full with no cache miss
- **TEST** t735 (`src/libs/kt-engine/tests/track_video.rs`)

### Requirement: Masks in the plan — the picture, an effect slot, a track; one raster per application
A mask list SHALL enter the plan as an internal chain slot, materialized by the engine as the leading
"Geometry" node and the morpher's strength are: the event's picture masks as an ALPHA slot right after
the leading Geometry node (the coverage rasterized in the node's output raster with the path mapped
through the node's placement — the one function the composite and the on-screen frame use — so the
mask is "crop → mask → placement" and follows the event's move, zoom and rotation); an effect slot's
masks as a MIX slot right after that slot (the slot's input is snapshotted, the slot runs, the two
are mixed per pixel by the coverage: inside the masks — the effect, outside — its input); a track's
masks as an ALPHA slot after the track's own chain, before its level and blend mode, in canvas space
(a track with masks is a track stage). The masks SHALL be evaluated at the frame's material tick when
the layer is built; the masks of the frame SHALL enter the plan key, and the raw masks with their curves the track stage's
content hash. The planner SHALL treat a mix slot as a strength stop and an alpha slot as a barrier;
neither MAY split a geometry series that has no mask in it. A slot before the leading node is masked
in the source raster, a slot after it in the canvas raster through the placement; a later Geometry
node or a distortion is not followed. The coverage SHALL be computed by the one kt-fx rasterizer once
per frame per application point and uploaded; the GPU SHALL apply those coverage bytes in an integer pass, so a
mask adds no divergence of its own: a mask-only frame equals its stored frame byte for byte, a masked effect
stays within that effect's own tolerance class. An event, a slot and a track without masks MUST get no slot: the plan, the
bytes and the cost are the earlier ones.
Why: the owner's S1 ("classic masks") within invariant 7; a canvas mask applied at the composite would
cover different content than a slot that runs before the Geometry node, and a GPU rasterizer would have
nothing exact to be held to (F-MSK-001).

#### Scenario: The picture mask cuts the event and follows its geometry
- **WHEN** an opaque event over a coloured lower track gets a rectangle mask on its left half; then the event's Geometry node moves it right by a quarter of the canvas; then the mask is inverted
- **THEN** inside the mask the event's colour, outside it the lower track's; after the move the visible region moved with the event by exactly the shift; inverted — the halves swap
- **TEST** t1270 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A slot mask limits an effect to the region
- **WHEN** an event with an invert-colour effect gets an ellipse mask on that slot; the same with the mask at 50 % opacity; the same slot placed before the leading Geometry node with the event zoomed
- **THEN** inside the ellipse the pixels equal the effect's, outside — the event's without the effect, byte for byte; at 50 % — the half-way mix; before the node the region is the same content of the source (it scales with the zoom)
- **TEST** t1271 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A track mask cuts the track's composite
- **WHEN** a video track with two events and a transition between them gets a rectangle mask; then the track's level is 0.5
- **THEN** outside the mask the lower layers are untouched, inside — the track's composite (the transition included); the level multiplies only inside; a track without masks keeps the earlier stage condition
- **TEST** t1272 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: Without masks — the earlier plan and bytes; a mask edit is not a stale frame
- **WHEN** a project without masks is composed; then a mask is added and its feather is changed; the frame is composed twice
- **THEN** without masks the chain has no mask slot and the frame equals the earlier path byte for byte (goldens untouched); each mask edit changes the plan key, the repeat is byte-identical
- **TEST** t1273 (`src/libs/kt-engine/tests/masks.rs`)

#### Scenario: A mask adds no divergence
- **WHEN** a picture mask after a rotated, zoomed Geometry node (opaque and alpha sources), masks alone on a track's chain, a slot mask before and after the node (a blur, a duotone) and a feathered Difference path are composed
- **THEN** the mask-only cases agree within `FrameTol::EXACT`; a masked effect within that effect's own class (`BILINEAR`, t181); and every case differs from the same plan without its masks
- **TEST** t1274 (`src/libs/kt-gpu/tests/masks.rs`)

### Requirement: The GPU preview's picture stays on the device
The preview SHALL compose through `VideoGpuExec::present`: the passes of
`execute_into`, then a display pass into a texture the UI draws, with no wait for the device and
no copy of the frame to the CPU. The display target SHALL hold the bytes the readback path shows,
premultiplied as `Color32::from_rgba_unmultiplied` does. An incomplete frame SHALL leave the target
as it is, unless nothing has been shown yet. An executor with no display target SHALL compose through
the readback path; with no executor there is no picture. Export SHALL keep its readback.
Why: at 1080p the UI thread spent about 33 ms of a 40 ms frame uploading, waiting for the device
and copying the frame out, before converting it and uploading it a second time (`kt-cli preview`,
llvmpipe; preview-throughput, 2026-11-04). The p95 stall per UI frame went from 36–55 ms to 7–11 ms.

#### Scenario: The picture does not change when the readback goes
- **WHEN** half-transparent events are presented and executed at the same moments, across a
  transition and over effects
- **THEN** the display target equals the readback's bytes premultiplied by egui's rule
- **TEST** t1525 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: An incomplete frame holds the last full one
- **WHEN** a starved frame is presented after a full one
- **THEN** the display target is unchanged, and with nothing shown yet the frame is drawn
- **TEST** t1526 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The glow and a new size
- **WHEN** a frame is presented with the glow, and then at another preview size
- **THEN** the glow holds the 32×18 picks of the readback's pixels, and the new size is a new
  texture with a new generation, while the same size keeps its generation
- **TEST** t1527 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The program draws from the device
- **PATH** open a project with a flat clip → the light frame shows → click the ruler over a clip with
  Edges → the frame goes black, and the program reports the picture was drawn on the device
- **TEST** live scenario `src/scripts/live/gpufx.py`

### Requirement: A strength slot at zero skips the layer, and the frame cost is counted
On a frame where the layer's strength slot evaluates to exactly 0, the plan executor SHALL
NOT execute the chain slice between the slot's snapshot and the slot. It SHALL return the snapshot (the dry
side), which is byte for byte what the mix at 0 gives. The
plan executor SHALL count, for each composed frame, the passes run, the sub-samples taken (the
pure sample cost of each geometry series), the snapshots and the mixes. It SHALL also count the
compose time. The counters are integers in the frame path: no allocation, no lock.
Why: ktprompts v26.10.18 item 8. Where a block's fades and gates put the layer's strength at
0, the executor still computed the whole event grid and then blended it away
(the `LayerMix` slices, the GPU Snapshot/Mix ops). There
was no number for "how much does this frame cost", so the question "do we compute extra?"
could only be answered by reading the code.

#### Scenario: Strength zero computes nothing of the layer
- **WHEN** an event with its own effect is covered by a layer whose chain is an event grid 12×30, with strength 0, 0.5 and 1.0 on successive frames
- **THEN** at 0 the frame equals the frame without the layer byte for byte (as t267 says) and the counters show zero passes and zero sub-samples for the layer's slice; at 0.5 and 1.0 the frames are unchanged from today; the counters at 1.0 show no snapshot and no mix
- **TEST** t945 (`src/libs/kt-engine/tests/morph_grid_cost.rs`), t945b (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: The source's proportions are one fact; a single picture is conformed like a video
Whether a source has proportions of its own SHALL be decided in exactly one place, from the
facts of the opened source. This decision SHALL be separate from whether the source has a frame grid of its own.
A single picture (JPEG, PNG, any file the system FFmpeg opens as an image) has no
frame grid (it is timed like a procedural source, as R.67 decided). It DOES have proportions:
its native size × pixel aspect. The plan SHALL conform it by the event's frame rule
(fit / fill / stretch / as is, the project default or the event's own) exactly as it conforms
a video of the same native size and pixel aspect: the same placement rectangle, the same
decode size, the same Geometry node. Only a source that draws its frame at the requested quality
(a generator, a title, the missing-file placeholder) SHALL have no proportions: its frame
is the canvas whatever the rule says. The composite, the frame drawn over the picture in the Geometry
window and the `kt-cli probe` conform line SHALL read this one fact. None of them may
decide it again on its own.
Why: ktprompts v26.10.18 item 9: "a square still on a 16:9 timeline gets squashed, and in
its Geometry not one frame-rule option works … why does it work for video and not for
stills?" R.62 equated "procedural" with "no proportions". R.67 then gave a still `fps: None`,
so the still became "procedural", and the renderer dropped its view. Every rule turned into
stretch. The overlay and the probe kept the view and showed a Fit frame that the render did
not use (a second arithmetic, forbidden since R.64).

#### Scenario: Four rules — a square still equals a square video
- **WHEN** a 180×180 still (`still_square.png`) and a 180×180 one-second video of the same lavfi pattern (`square.mp4`) are each placed on a 320×180 canvas, and the event rule is set in turn to fit, fill, stretch and as is (and once "project" with the project default changed)
- **THEN** for every rule the still's placement rectangle and decode size equal the video's, and equal the analytical ones: fit — 180×180 centred with transparent side margins; fill — 320 wide, cropped top and bottom; stretch — the whole canvas; as is — 180×180 centred 1:1. The composed frames of the still and the video give the same white-rectangle geometry (a pixel probe of the edges, ±1 px for the codec)
- **TEST** t937 (`src/libs/kt-engine/tests/stills.rs`)

#### Scenario: One decision — a still has a view, generators and the placeholder do not
- **WHEN** the source facts observe a single picture (square, and once with SAR 2:1), a video, a generator, a title and the missing-file placeholder
- **THEN** the picture is timed as procedural (no floor to the frame grid, no end of material: t796b unchanged) AND has its view (width × SAR, height); the video has its view and is not procedural; the generator, the title and the placeholder are procedural and have no view. The renderer's lead context, `Player::source_view` and the probe's view are the same value for each of them
- **TEST** t936 (`src/libs/kt-engine/tests/conform_rule.rs`)

#### Scenario: The executor places the still by the rule
- **WHEN** the square still is composed with the fit rule over a coloured lower layer (the lead node built by `src/libs/kt-fx/src/geom.rs` and executed by `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)
- **THEN** the side margins are transparent (the lower layer shows through); the frame is within the tolerance class of t713 of its stored frame; with the stretch rule it stays within that class of the frame the still gave before this change
- **TEST** t938 (`src/libs/kt-gpu/tests/gpu_render.rs`, `src/libs/kt-fx/src/geom.rs`, `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)

#### Scenario: The frame over the picture is the frame of the render
- **WHEN** a project with a 320×180 canvas and an event on the square still is opened in the harness, the event is selected, its Geometry form is opened and the frame rule is switched through fit, fill, stretch and as is
- **THEN** after each switch the placement rectangle that the on-screen frame draws equals the rectangle by which the composite placed the layer (checked against the preview frame's pixels), and the preview changes: pillarbox margins for fit, cropping for fill, the whole canvas for stretch, a centred 1:1 square for as is
- **PATH** drag `still_square.png` onto the timeline → the event appears, the preview shows the square picture with transparent side margins (project rule fit) → open the FX window of the event → the Geometry node is first, its rule row shows five words (project / fit / fill / stretch / as is) → click **fill** → the picture fills the width and its top and bottom are cut → **stretch** → the picture is squashed to 16:9 → **as is** → a 180×180 square in the centre → right-click the preview, turn the on-screen controls on → in every step the frame drawn over the preview matches the picture's edges
- **TEST** t939 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/stills.py`

#### Scenario: "As is" counts the pixels of the composite's raster, not of the preview canvas
- **WHEN** a 360×360 picture stands under "as is" in a 1280×720 project, and the frame of every rule is computed for the preview at the fractions 1, ½, ¼, ⅛ and 1/16 of the project (the canvas is then 640×360, 320×180, …), with the composite's raster (the project, or a reel's own raster) carried in the lead context
- **THEN** the frame of every rule, "as is" included, is the same in fractions of the canvas at every fraction — 360/1280 of the width and 360/720 of the height for "as is" — so the preview shows the size the file gets; when the canvas IS the raster (export at the project size) the frame is the view itself, byte for byte as before, and the GPU plan key carries the raster
- **PATH** a 1280×720 project with a 360×360 red picture, preview at ½ → open the FX window of the event, click the Geometry node → click **as is** → the red square is half the height of the preview frame (not its full height), and the on-screen frame sits on its edges
- **TEST** t765 (`src/apps/kt-app/src/app.rs`, extended from fit to all four rules), live scenario `src/scripts/live/stills.py`

#### Scenario: The probe tells the same truth
- **WHEN** `kt-cli probe` is run over a project with the square still under fit and the placeholder of a missing picture
- **THEN** the conform line of the still prints `view=180x180` and the fit frame the renderer uses; the placeholder prints `unknown/stretch`; both lines come from the same function the renderer calls
- **TEST** t940 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Procedural sources and the goldens do not move
- **WHEN** the generator, title and placeholder scenarios and the golden exports are run after the change, and a missing picture is shown in a 4:3 project
- **THEN** generators and titles are still conformed as an identity (t471 green), the placeholder still fills the whole canvas in 4:3 (no margins), t796, t796b, t796d, t715 and `GOLDEN_VIDEO` are green with no regeneration
- **TEST** t941 (`src/libs/kt-engine/tests/stills.rs`)

### Requirement: A live preview paces a stream stage's history
In the live preview (`StreamPacing`, set by the player on its renderers; `None` for an export and every
other path, whose bytes do not move) the order of a stream layer SHALL be the earliest positions its
history lacks that fit a quarter of the video cache, at least one, in the order the stage reads them. The
prefetch thread SHALL advance the stream stages over what the decoder brought — at the frame on screen and,
while playing, ahead of it (wrapping in the loop region) by the frames the display reaches before its next
pass, twice over, bounded by a quarter of the stage's memo — and a pass that leaves a stage pending SHALL
run again even on a pause. A composite SHALL run a stage only when its frame lacks at most two positions:
on a pause it reports the frame incomplete; while playing it shows the latest frame the stage holds on that
frame's stream, and such a composite is not kept as the moment's own. An offline path with its own sources
and a small cache (the reel strip, the project poster) SHALL pre-roll a stream stage as the export does, in
batches of half its cache.
Why: ordering the whole history (up to 600 frames of 8 MB at 1080 lines against a 512 MB cache) let the
decoder run through it while the stage read only at composites; the cache dropped the history's front, the
frame never came, and the owner saw a loop at V-fps 0. With the history cached, the composite computed it
on the UI thread: 9 s for one composite on his relinked project (decode-request-storm).

#### Scenario: The paced order is the earliest of the history
- **WHEN** a frame 3 s into a cut mosh (a ~100-frame history) is ordered with budgets of 1, 7 and 30 frames, and of one byte
- **THEN** each order equals the first 1, 7, 30 and 1 entries of the unpaced order
- **TEST** t2042 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The advance computes the history, not the composite
- **WHEN** every frame of that history is cached and a paced renderer composes the frame, then runs `stream_advance`, then composes again
- **THEN** the first composite is incomplete and the memo untouched; the advance reports nothing pending; the second composite is complete and byte-identical to the unpaced renderer's; the next frame, one step on, is the composite's own
- **TEST** t2043 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: A frame creates no GPU resource after warm-up
After its first frames (new pipeline shapes, the pool's textures, the uniform arena, the pass timer's
buffers), composing a frame SHALL create no texture, buffer or pipeline: transients come from the pool,
uploads from the upload cache's spares, readback buffers are kept. The one exception is the preview's
memo of presented composites (cached-range-plays), which keeps each new complete composite until its
budget is full and then recycles the evicted ones: at most one texture a new presented frame. The compute
host likewise reuses its buffers and pipelines job after job. `kt_gpu::resources_created()` counts every
creation at the sites a frame can reach.
Why: gpu-pass-cost — a creation per frame is an allocation, often a cleared one, on the card's memory
every frame; on an integrated GPU that memory is the system's.

#### Scenario: Every effect and preset composes without creating anything
- **WHEN** every video effect of the registry at its defaults and at each preset, and the heavy stack (VHS Tape, Camera ISP, glow, radial blur over two more layers), compose three warm frames at 1080p through the readback path, and three more through the present path
- **THEN** the readback frames created nothing, and the presented ones at most one texture each
- **TEST** t2023 (`src/libs/kt-gpu/tests/no_creation.rs`)

#### Scenario: A device that returns its timestamps late makes the timer no buffer
- **WHEN** a timed frame is composed again and again while none of the earlier frames' timestamp maps has come back (a loaded machine; in the test nobody polls)
- **THEN** the timer's read buffers are the PENDING_MAX it was made with: the frames past them run untimed, nothing is created, and once the maps return the frames are timed again
- **TEST** t2025 (`src/libs/kt-gpu/src/pass_time.rs`)

#### Scenario: A Datamosh frame reuses the compute host's buffers
- **WHEN** a warm Datamosh encoder job and decoder job run three times
- **THEN** no buffer or pipeline was created
- **TEST** t2024 (`src/plugins/fx-video-codec/tests/compute_reuse.rs`)

### Requirement: The kept moments are checked against an edit off the UI thread
The bars' "plays without work" layer SHALL be read on the UI thread from the map of kept moments
alone. Moments presented before an edit SHALL be checked against the current plan on the preview's
own thread, between frames, a few per pass and yielding to a frame that is asked; the executor's memo
SHALL be asked with the map free. While moments wait, the query says the check is pending and the
preview's thread wakes the UI when it moves.
Why: the check builds frame plans, and a plan with a stream stage runs its kernel (compute jobs read
back with a blocking poll); `holds` takes the engine's lock, which the preview's thread holds for a
whole composite. Both were asked by `coverage_runs` on the UI thread.

#### Scenario: The bars while the executor's memo is held
- **WHEN** a presented moment is on the bars, the executor's memo is held, the project is edited, and the bars are asked and frames asked three times
- **THEN** every query returns at once, says the check is pending and no longer draws the moment as kept; once the memo answers the moment comes back
- **TEST** t1843 (`src/libs/kt-engine/tests/compose_queue.rs`)

### Requirement: A video build's work grows linearly with the project
`VideoRenderer::new` (and `gen_sources`, which walks the same instance tree) SHALL do work linear in
the project, by the same rule and the same counting guard as the audio build: transition ends and
their engine events by index, a reel's revision once per build (`revisions_scope`), the profile of a
speed curve once per event.

#### Scenario: Events, transitions and placements
- **WHEN** the generated scenes of the audio build's guard (events with transitions, a long signed speed curve, placements, nested placements, a cutting layer, linked curves) are built ×1, ×4, ×16
- **THEN** the video build and `gen_sources` grow at most 8× and 32× in walked elements and in allocated bytes
- **TEST** t2062 (`src/libs/kt-engine/tests/build_cost.rs`)

### Requirement: Every device of the process opens from one instance, one at a time
Every headless device (export engines, the offline engine, kt-cli), the compute host and the
adapter list SHALL come from one `wgpu::Instance` per backend set, created at the first ask under one
process-wide opening lock and never dropped; devices SHALL be opened one at a time; the adapter list
SHALL be taken once per process, and in the editor before eframe opens its own instance. An export
run with no `DISPLAY` and no `WAYLAND_DISPLAY` SHALL finish.
Why: found-defects 131 — `env -u DISPLAY kt-cli export … libx264` died by SIGSEGV every run:
`export_engines` opened two to four devices on threads at once on a software adapter, each creating
its own instance, and the Vulkan loader's instance-level calls raced (`vkEnumerateInstanceExtension
Properties` beside `vkCreateInstance`, gdb). The load watch re-created a whole instance — every ICD
loaded, every driver's configuration parsed — each five seconds of a hot spell, beside the preview.
Found-defects 132 — t985 died by SIGSEGV now and then under nextest: a generator's first shader
frame opened the compute host on a media worker while the poster's offline engine opened (the
native-crash report named `media-worker-46` in `vkEnumerateInstanceExtensionProperties` ←
`kt_gpu::compute::open_device`); on the old code 33 of 60 runs, and t1312, t986, t958 the same way.

#### Scenario: Export with no display, four devices
- **WHEN** kt-cli exports a short project to libx264 three times with `DISPLAY` and `WAYLAND_DISPLAY` removed and `KT_VIDEO_ENGINES=4`
- **THEN** each run exits 0 and writes a file (before: exit 139 at the first run)
- **TEST** t2086 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Devices opened at once share the one instance
- **WHEN** four threads open an export device at once while two more open the compute host and list the adapters, three times
- **THEN** every device opens and the process created at most two instances (the headless devices' backend set and the compute host's primary one)
- **TEST** t2087 (`src/libs/kt-gpu/tests/one_instance.rs`)
