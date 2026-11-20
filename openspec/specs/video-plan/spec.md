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
A composed frame SHALL be memoized in the preview as a function of the whole plan, keyed by the plan key: on the readback path in the executor's readback memo, on the display path in the executor's present memo (`PresentMemo`), where presenting a held key runs only the display pass (`GpuStages.replayed`, the work row `composite_replays`). Only complete frames are kept. The present memo SHALL keep its 512 MB base and, while a loop region is set, SHALL want every composite of the region at the size it presents, plus an eighth; the memory ledger grants that want before any other tenant's growth (media-index). Without a loop or a grant it is bounded by its base, least recently shown first. The key SHALL include the hash of the chain with its
curves, the geometry of the clip, the resolved camera (it is selected per frame and is not visible to
a static hash), the masks of the frame, the decode step of the input, the parameters of the
transition kind, the step of the reel and the blend flag. Export MUST NOT memoize (each t is
composed once).
Why: any parameter that affects the bytes and is not in the key is a stale value, which shows up as "did not apply". preview-throughput D2 moved the preview to the display path, which had no memo: a covered range was composed again on every lap — 16 V-fps on the owner's project (kkt-26.11.07b, line 3). found-defects 310: the constant 512 MB held 582 composites at 640×360 and 60 at 1080p, and the owner's region is 665 frames, so its laps composed again every time (memory-ledger).

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

#### Scenario: A granted loop is kept whole past the base
- **WHEN** the present memo's base holds ten composites, a loop of thirty moments is announced and granted, and the loop is presented three times
- **THEN** the second and third laps compose nothing and show every moment from the kept composites, the same pictures as the first; the memo holds the thirty past its base; the loop cleared, it is back at its base at once
- **TEST** t2292 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The player asks for the loop's composites
- **WHEN** a loop region is set on a playing player with a display executor, the preview size changes, and the region is cleared
- **THEN** the executor is told the region's moment count each time it changes, its want in the ledger is the region's composites at the presented size plus an eighth, and with no region the want is the base
- **TEST** t2296 (`src/libs/kt-engine/tests/memory_ledger.rs`)

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
as it is, whether or not anything has been shown, and SHALL cost no drawing work: a plan with a layer
lacking its frame (`FramePlan::starved`) SHALL stop before its first upload, any other incomplete plan
after its resolve, with no pass encoded and no display pass. `execute_into` SHALL likewise leave an
incomplete frame's bytes empty with nothing uploaded, encoded or read back; only `execute_once` (an
export's last resort, named in its report) SHALL produce an incomplete frame's bytes. An executor with no display target SHALL compose through
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
- **WHEN** a starved frame is presented after a full one, with and without the glow
- **THEN** the display target is unchanged both times (no option draws an incomplete frame)
- **TEST** t1526 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: An incomplete frame costs no drawing
- **WHEN** a starved plan is presented, executed through `execute_into` and executed through `execute_once`
- **THEN** the present and `execute_into` upload no byte, submit nothing, run no pass, read nothing back and `execute_into` leaves no bytes; `execute_once` returns the frame's bytes, reported incomplete
- **TEST** t2530 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: A plan knows its starvation
- **WHEN** a frame's plan is built over a cold cache, over a cache holding every layer but one, and over a warm one
- **THEN** `FramePlan::starved` is true, true, false
- **TEST** t2531 (`src/libs/kt-engine/tests/no_partial.rs`)

#### Scenario: The glow and a new size
- **WHEN** a frame is presented with the glow, and then a complete frame at another preview size
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
run again even on a pause. A composite SHALL run a stage only when its frame lacks at most two positions;
otherwise, playing or paused alike, it reports the frame incomplete and says how many positions the
history still lacks (the most over the frame's stream stages, reels and track stages included). An
offline path with its own sources and a small cache (the reel strip, the project poster) SHALL pre-roll a
stream stage as the export does, in batches of half its cache.
Why: ordering the whole history (up to 600 frames of 8 MB at 1080 lines against a 512 MB cache) let the
decoder run through it while the stage read only at composites; the cache dropped the history's front, the
frame never came, and the owner saw a loop at V-fps 0. With the history cached, the composite computed it
on the UI thread: 9 s for one composite on his relinked project (decode-request-storm). The playing
preview once showed the stage's latest held picture instead; the owner decided against it (kkt-26.11.10b,
found-defects 403: "ну пусть loading показывает").

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
run with no `DISPLAY` and no `WAYLAND_DISPLAY` SHALL finish. A device on the GL backend SHALL open
from an instance of its own (still under the opening lock, dropped with its device): wgpu's GL
adapters of one instance share its one EGL context, and two devices on it cannot run on two threads.
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

#### Scenario: On GL the export opens its engines beside a live device
- **WHEN** under `WGPU_BACKEND=gl` the offline engine is open and `export_engines` opens four engines (`KT_VIDEO_ENGINES=4`), twice
- **THEN** every engine builds and the process lives (before: `GpuFxEngine::build` panicked on EGL_BAD_ACCESS — the shared instance's one EGL context was current on another thread — and the unwind aborted dropping the queue; every render from the editor in the live `render`, `renderregion`, `hwencode` scenarios died so)
- **TEST** t2088 (`src/libs/kt-gpu/tests/gl_devices.rs`)

### Requirement: A frame never compiles; what a project reaches is warmed beside it
When a project is opened, when an executor arrives and when an edit brings an effect into the project,
the player SHALL hand the effect and generator ids the project reaches (clips, tracks, morpher
patches, compositions) to the executor's `prewarm` and to the compute host's; an edit that leaves the
set as it was SHALL ask nothing. `prewarm` SHALL return at once: the device's warm worker (a thread
started when work arrives and ended when its queue is empty) draws each reached pipeline once into a
few-texel target — the core (the fixed pipes, the display pass, the geometry interpreter) the first
time, then each effect's own passes — so a driver that compiles at the first draw (llvmpipe, lavapipe)
has compiled them before a frame draws them. Until a pipeline is warm a frame draws it as before. The
worker SHALL submit one pipeline at a time, each after the frames composing on the device, and on a
software rasterizer whose frames a person watches it SHALL warm a shadow device of the same adapter
first, so the watched device's warm draws are loads from the driver's cache: a frame never stands
behind a warm-up's compile. A
geometry shape a frame meets without a warm pipeline of its own SHALL run the interpreter (the same
bytes) while the worker compiles and warms the shape; a frame SHALL create no pipeline. An export
SHALL prewarm every device it composes on and wait for the worker before its first frame. Background GPU work (the warm
worker and its shadow device, the compute host's opening and preparation, an engine compiled on a
thread) SHALL hold the process's exit: at `exit` it starts nothing more and the exit waits for the draw
or compile in flight, so nothing is on the device when libLLVM's destructors run. The pipelines
of the editor's display device SHALL be kept for the process (`new_kept`), so an engine built again
for a project opened again takes them warm, and the editor SHALL build its engine on a thread.
`kt_gpu::cold_draws()` counts a pipeline drawn or dispatched in a frame before it was warmed,
`kt_gpu::pipelines_in_frame()` a pipeline created on a frame's path.
Why: engine-audit M8, found-defects 64 and 139 — measured with `kt-cli first-frame` on llvmpipe with an
empty Mesa cache: the first composite of all 10.5 s, Camera ISP's first frame 4.6 s, VHS Tape's 1.2 s,
7.2 s of first-frame excess over every effect, while creating every pipeline took 1.9 s; a new
geometry shape compiled inside its frame; the editor built its engine on the UI thread at every project
open (0.95–1.9 s). After: excess 149 ms over every effect, no cold draw. A first version drew the whole
warm-up in one submission, and lavapipe's one queue thread made the showcase's first moment wait
behind it (16.0 s against 12.8 s).

#### Scenario: After the prewarm no frame compiles
- **WHEN** an engine is prewarmed with every effect id and the heavy stack, then every video effect at its defaults and each preset, and the heavy stack, compose at 1080p through the readback and the present paths
- **THEN** no frame draws a pipeline that was not warmed and none creates a pipeline; a second prewarm of the same ids starts nothing
- **TEST** t2120 (`src/libs/kt-gpu/tests/prewarm.rs`)

#### Scenario: A new geometry shape compiles beside its frame
- **WHEN** a frame meets a geometry shape (an affine series, a Pan3d, both) no frame met before, on an engine whose core is warm
- **THEN** the frame draws nothing cold and creates no pipeline; once the worker settles the shape has its own pipeline, and the next frame gives the first frame's bytes
- **TEST** t2121 (`src/libs/kt-gpu/tests/prewarm.rs`)

#### Scenario: The process exits while a warm-up compiles
- **WHEN** a process starts the warm-up of every effect and every compute shader on an empty Mesa cache and calls `exit` while the work is on the device, three times
- **THEN** each exits with status 0: the exit stops the queues and waits for the draw or compile in flight (before: SIGSEGV, status 139, in libLLVM's destructors — row 172)
- **TEST** t2129 (`src/libs/kt-gpu/tests/exit_while_warming.rs`)

#### Scenario: A prewarmed GL engine keeps composing
- **WHEN** on the GL backend a software engine is prewarmed with every effect, composes a frame, and another engine opens and composes
- **THEN** both frames compose, and the first draws nothing cold
- **TEST** t2128 (`src/libs/kt-gpu/tests/prewarm_gl.rs`)

#### Scenario: The player asks only when the reach changes
- **WHEN** an executor arrives on a project with Glow on a track, the project is edited without a new effect, then a composition with VHS Tape inside is added
- **THEN** the executor is asked once with Glow on arrival, not at the plain edit, and once more with Glow and VHS Tape
- **TEST** t2127 (`src/libs/kt-engine/tests/prewarm_reach.rs`)

### Requirement: Compiled pipelines have a disk tier where the device keeps one
Where the device has `Features::PIPELINE_CACHE` (wgpu offers it on Vulkan), every pipeline of the
engine and of the compute host SHALL be created with one `PipelineCache` per device, read from and
written to `<data>/shader-cache/<key>.bin`, where the key hashes the adapter (vendor and device), the
driver and its version, the wgpu version, the part of the program and every shader text it compiles.
The blob SHALL be written after the warm worker empties its queue (the compute host: after a
preparation) when it grew, through a temporary file and a rename; the folder SHALL keep the four newest
files and no blob above 64 MB. The frames SHALL be the same bytes with and without it. With no folder
set (tests) or no feature (GL, DX12, Metal) nothing changes.
Why: each new device recompiled everything (`cache: None`). On lavapipe the blob is its 96-byte header
(it keeps no binaries; Mesa's own disk cache serves it); a card's driver fills it.

#### Scenario: The cache changes no byte
- **WHEN** three engines on three devices compose every video effect: with no cache folder, writing the cache after a prewarm, reading it back
- **THEN** their frames are equal, and where the device has the feature the file is in the folder
- **TEST** t2125 (`src/libs/kt-gpu/tests/pipeline_cache.rs`)

#### Scenario: The key and the bound
- **WHEN** the key is taken for another driver version, another shader hash and another part, and seven blobs lie in the folder
- **THEN** each key differs, the same inputs give the same key, GL has none, and trimming keeps the four newest blobs and every other file
- **TEST** t2126 (`src/libs/kt-gpu/src/pipeline_cache.rs`)

### Requirement: A kept moment is reached without the work that made it
The preview SHALL ask the display executor for a moment's kept composite BEFORE it asks anything of the
cache, the decoders or a stream stage: the key comes from a probe plan in which every stream stage gives
the key its kernel names for the frame (`StreamKernel::key`, computed from the rows and the source
addresses, nothing rendered, the memo not read), and a held key is shown by the display pass alone
(`present_kept`). The kept-moment recheck SHALL use the same probe plan. A cache entry evicted and
decoded again from an unchanged source SHALL take back its content serial (invalidating the source
forgets it). A complete composite of the slow-chain device SHALL be kept on the display's executor, and
a moment shown from it does not re-time the route. The look-ahead SHALL ask neither decodes nor stream
history for a moment the bars show as kept. Applies to every effect with history and every way of
reaching a moment: pause, slow rate, loop, scrub.
Why: the owner, kkt-26.11.09 I2/I5/I8: "если хоть сто раз пройтись по небольшому куску - он все
равно что-то loading делает". The trace of the heavy loop (Camera ISP → Datamosh → VHS Tape) showed
every kept composite costing a full plan with its Datamosh stage (15 ms per composite against 0.3 ms of
replay), laps composed again for 16 s because the stage showed held frames, and — where the memo had
dropped the history — a kept frame planned short and shown as "Loading".

#### Scenario: A kept moment needs no history
- **WHEN** a paused frame under a cut mosh is composed whole, then the stream memo is cleared and both sources' cache entries are invalidated, and the frame is presented again
- **THEN** it is complete at once from the kept composite: no stream stage ran for it, no decode was asked, it waited for nothing, and the bars show the moment as kept
- **TEST** t2145 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The look-ahead skips kept moments
- **WHEN** the look-ahead is asked with ten moments kept
- **THEN** none of their keys is asked and every other key is asked as before
- **TEST** t2150 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The slow route keeps its composites
- **WHEN** a moment of a chain slower than `SLOW_CHAIN` is composed on the slow device as bytes, then asked again, then a neighbouring moment is asked
- **THEN** the second ask is shown on the display from the kept composite (one replay), and the neighbour still goes to the slow device
- **TEST** t2151 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A re-decoded entry keeps its serial
- **WHEN** an entry is evicted and inserted again, then overwritten, then its source is invalidated and it is inserted again
- **THEN** the re-insert has the serial it had, the overwrite and the insert after the invalidation have new ones
- **TEST** t2152 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: A short loop under a heavy chain plays from what is kept
- **PATH** open our showcase with the owner's chain (Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape) on the longest footage event and a 0.7 s loop over it → the paused frame is shown → press Space → the first laps compose, then the strips over the loop turn bright → no frame is composed, no burst runs the effects, nothing is decoded → press Space → the paused frame inside the loop is there, no "Loading"
- **TEST** t2153 live scenario `src/scripts/live/heavy_loop.py`

### Requirement: The preview's video renderer is the shared build's
The preview composer, prefetch and the UI's readers SHALL use the video renderer of the player's
shared build; a video layer reused from the last build SHALL equal the freshly derived one (the plan
key, the decode step and the stream stage included).
Why: the UI thread built a `VideoRenderer` per edit (found-defects row 150, 19–36 ms on the owner's
project), and prefetch built another.

#### Scenario: The video side of an incremental build
- **WHEN** the edits of t2004 are applied in turn
- **THEN** the incremental video build's state text (every layer, the cameras, the stages, the nested reels) equals the fresh build's
- **TEST** t2004 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: A run of point-wise stages is one generated pass
`plan_chain` SHALL emit `PlanOp::PointRun { i, j, res_scale }` for two or more consecutive chain
stages at a barrier position that each fuse in runs (`kt_plugin_api::fuses_in_runs`: class
`PointWise`, a `tap_<id>` WGSL, no barrier kernel); audio slots, stream stages and identity Geometry
nodes between them are transparent. An active geometry step, a barrier, a point-wise plugin with a
barrier kernel of its own (noise, greenscreen), a layer-strength slot and a stop (a strength snapshot,
a mask point) SHALL end a run; a lone stage SHALL stay a `Barrier` op. A run takes the resolution its
first stage would have taken as a barrier (the K× rule unchanged).

The GPU executor SHALL run a `PointRun` as ONE pass generated for the run's shape (the sequence of
its plugin ids): one texel load, each stage's `tap_<id>` in chain order with the stage's numbers from
the pass's uniform, the stage's rounding and clamp to 0…255 after each stage — what the stage's own
pass stores — and one store. A run longer than the uniform's room (`MAX_RUN_STAGES` = 32) SHALL be
cut into pieces of that room. A shape's pipeline SHALL be compiled and warmed on the device's warm
worker (a frame never compiles); until it is warm, and wherever the executor splits it, the stages
SHALL run as their own passes. A warm shape SHALL be tried against its own passes: the frames of the
trial SHALL show the own passes, and the fused pass SHALL run beside them into a scratch target, both
timed by the pass timer in the same frame; when the fused pass is slower per pixel than the sum of its
parts (minima of the tries) the shape SHALL be split into two halves, each tried again, else the frames
SHALL show the fused pass from then on. On a device without timestamps a warm shape SHALL run fused.
The output form of a frame SHALL be a function of its shapes' states alone, never of the trial's
alternation; a process-wide forms revision SHALL move whenever a shape's output form can change (a shape
compiled, a verdict) and SHALL be part of the readback memo's and the present memo's key, and a composite
whose resolve saw the revision move SHALL NOT be kept. An export's frame (`execute_once`, the readback
ring's `submit`) SHALL take the SETTLED form: every run's generated pass, compiled in the frame when not yet
compiled, no trial and no split; a text the device refuses runs as its own passes. The player SHALL hand the run shapes a project reaches to
the executor's `prewarm_runs` with its effect ids.

The fused and the own-pass forms SHALL give frames within 1 LSB per former boundary (the owner's
agreement, H16); on the reference software adapter they are byte-equal. On a card they are bounded, not
equal, so no kept or exported frame depends on which form a timing chose (found-defects 510).
Why: engine-audit M7, row 10: each point-wise effect read and wrote the whole frame (8 bytes a pixel,
2–3 ms at 1080p on llvmpipe, gpu-pass-cost D2). Measured after: design D-measure.

#### Scenario: The plan names runs and ends them where the class says
- **WHEN** chains of point-wise, geometry, barrier, noise/greenscreen, audio, strength and mask stages are planned, with and without stops and at K×
- **THEN** every two or more consecutive fusable stages form one `PointRun`, a lone one stays a `Barrier`, every other class ends a run, and the K of a run is the K its first stage had
- **TEST** t2191 (`src/libs/kt-fx/tests/point_runs.rs`)

#### Scenario: Fused equals unfused over every pair and a long run
- **WHEN** every ordered pair of fusable point-wise plugins (defaults and a tuned value set), a six-stage run, a 40-stage run (over the room) and a run inside a strength slot and beside a geometry series compose at 1080p fused, and again with fusion off
- **THEN** each frame is within 1 LSB per former boundary of its unfused frame, and on a software adapter byte-equal; the fused frame runs one pass per run piece
- **TEST** t2190 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: A run shape compiles beside its frame
- **WHEN** a frame meets a run shape no frame met, on an engine whose core and effects are warm
- **THEN** the frame runs the own passes, draws nothing cold and creates no pipeline; after the worker settles the shape is ready and the next frame runs one pass with the first frame's bytes; a shape handed to `prewarm_runs` is ready before its first frame
- **TEST** t2192 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The switch is measured on the device
- **WHEN** a trial records fused passes slower than the sum of their parts for a four-stage shape, then its halves faster
- **THEN** the shape is split into the two halves, each half is tried and kept fused; with faster fused tries the whole shape stays fused; with no timestamps it is fused at once
- **TEST** t2193 (`src/libs/kt-gpu/src/runs.rs`)

#### Scenario: Preview and export agree on a run
- **WHEN** a project with a point-wise run is exported and the same moment composed for the preview, on a fused engine and on one with fusion off
- **THEN** the four frames are byte-equal on a software adapter
- **TEST** t2197 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The player prewarms the project's run shapes
- **WHEN** a project's track holds Color Grade, Posterize and Vignette, then a clip a run of two inside a strength slot, then an edit adds nothing
- **THEN** `run_shapes` gives the three-stage and the two-stage shape, and the executor is asked for the runs again only when the set changed
- **TEST** t2196 (`src/libs/kt-engine/tests/prewarm_reach.rs`)

#### Scenario: A trial never changes what a frame shows
- **WHEN** a three-stage shape no frame met is composed until its trial's verdict
- **THEN** the first frame runs its own passes; every frame of the trial shows the own passes (the fused pass timed beside them) with the first frame's bytes on a software adapter; after a fused verdict the frame shows the fused pass; the forms' revision moved
- **TEST** t2514 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The memo key names the run forms
- **WHEN** a plan is composed twice through the readback memo, then a new run shape is compiled, then the plan is composed twice again
- **THEN** the second ask is a hit; after the shape compiled the plan is composed again, then kept again
- **TEST** t2515 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: An export's frame runs the settled form
- **WHEN** an export frame meets a run shape no frame met, on an engine in the measured mode, four times
- **THEN** every time the run is fused (compiled in the first frame), the bytes are the same every time and within 1 LSB a boundary of the own passes (equal on a software adapter)
- **TEST** t2516 (`src/libs/kt-gpu/tests/fused_runs.rs`)

### Requirement: One arbiter per ring orders every device's submissions by class
Every device the program opens (the display's, the offline engine's, the export's, the compute host's)
SHALL register on the ring of its adapter: the devices of one hardware adapter share one ring whatever
their backend; a software rasterizer's device is a ring of its own. Every submission except the
display's present SHALL be admitted by its ring, the waiting submission of the highest class first
(`now`, `answer`, `near`, `visible`, `background`, `offline`), and no submission of a class is admitted
while one of a higher class waits. The class is the one its caller runs as (the compose thread: now;
the prefetch thread: near; a scheduler job: its lane's class; an export: offline); an untagged thread
is background. Admission changes timing only: the passes, their order and their bytes do not change
(invariant 11).
Why: `docs/engine-audit.md` M5, rows 134 and 149: the devices of one card share its ring and the
compute host ran whoever submitted first, so the frame on screen waited behind look-ahead and offline
work.

#### Scenario: The highest waiting class is admitted first
- **WHEN** a ring holds a piece in flight and an offline, a near and a now submission wait for it
- **THEN** the now submission is admitted when the piece completes, then the near, then the offline, whatever order they came in
- **TEST** t2250 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: Devices of one card share a ring; a software device does not
- **WHEN** two hardware adapters' facts name the same card on Vulkan and on GL, and two llvmpipe devices are opened
- **THEN** the two hardware devices register on one ring, and each llvmpipe device on a ring of its own
- **TEST** t2252 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A thread's class follows its work
- **WHEN** a scheduler lane of class visible runs a job, a thread sets class near for a block, and a fresh thread asks
- **THEN** the job sees visible, the block near and the thread outside it its own, the fresh thread background
- **TEST** t2256 (`src/libs/kt-time/src/sched/tests.rs`)

### Requirement: A ring that carries a display holds at most one bounded piece in front of it
On a ring that carries the editor's display device, the lower-class work (every class below `now`) that
the display's NEXT submission would find in front of it SHALL stay under the ring's bound. The ring
predicts its queue: every admitted submission carries its predicted device time and, once submitted, its
submit instant, and the queue drains in submission order (a submission past its predicted end and not yet
done ends now); the display's next submission is expected one frame period after its last while it
presents, else now. A lower-class submission SHALL be admitted while the predicted drain with it ends no
later than the bound past the later of the display's next submission and the end of the display's own frame
in flight; every lower class has the same bound, and no count of lower submissions limits it. A `now`
submission keeps one bound of predicted time in flight, and one piece is always admitted on an idle ring.
The bound is ONE frame period of the preview's project (40 ms at 25 fps), a general rule; a lower-class
frame or compute job SHALL be cut at pass (dispatch) boundaries into pieces whose predicted device time
stays under HALF the bound (a pass longer than that is a piece of its own), and no admission SHALL claim
more than half the bound, so one piece is admitted behind the one that runs while what stands in front of a
present stays within the bound. A ring with no display neither bounds
nor cuts: an export's ring of frames and a software device keep their pipelining. The cut frame's bytes
SHALL equal the uncut frame's.
Why: row 149 — on a real card the display and the offline device share the graphics ring, and one
long submission held the display. Found-defects 480 — waiting for the earlier piece's completion cost a
Datamosh walk 96–98 % of its wall on his iGPU. The bound was half a frame period until 2026-11-17; two
designs called it "the owner's latency limit", which he never set (gpu-arbiter D3: "a general rule … the
owner may want to revisit"). Asked with context, he raised it ("Поднять", kkt-26.11.17 S4): a lower piece
may delay a displayed frame by up to one period, and while playing the display presents kept frames only. Pieces were cut at the whole bound from then until ahead-kept: a piece that size fitted only an EMPTY
ring, so every piece of the history walk waited for the one before to end, and the owner's card was busy
32–46 % while a VHS Tape + Camera ISP + Datamosh loop missed frames (kkt-26.11.18 T2: a history position's
59 ms of device took a 117 ms span).

#### Scenario: The in-flight bound applies only with a display
- **WHEN** a ring with a display holds pieces whose predicted drain reaches the bound, and the same pieces wait on a ring without one
- **THEN** the displayed ring admits the next only when the drain leaves room for it (one piece always fits an idle ring), the other admits them all
- **TEST** t2251 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A long frame is cut on a displayed ring with the same bytes
- **WHEN** a heavy chain (VHS Tape, Camera ISP) is executed twice at 1080p by an offline engine on a device that also carries a display, once warm with its passes measured
- **THEN** the measured run is submitted in more than one piece, no piece but a single pass predicted over the bound, and its bytes equal the uncut run's
- **TEST** t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The display's latency on a shared device is bounded
- **WHEN** the preview presents a light frame every frame period on one device while an offline heavy chain and a Datamosh history run on the same device, with the arbiter and without it
- **THEN** with it the display's frame waits for at most one bound of lower work (its latency stays below the offline frame's own device time); without it the display waits for whole offline frames
- **TEST** t2254 (`src/libs/kt-gpu/tests/arbiter.rs`), bench `kt-cli gpu-share`

#### Scenario: The compute host admits the frame on screen first
- **WHEN** a near-class Datamosh job runs on the compute host and a near and a now job wait
- **THEN** the now job is admitted next, and every job's admission is in the trace with its class
- **TEST** t2255 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A lower class keeps a piece queued while one runs
- **WHEN** a displayed ring idle of display work takes lower-class pieces of a quarter of the bound each, none completing
- **THEN** four are admitted at once (their drain fits the bound from now), and the fifth waits until the drain has room
- **TEST** t2430 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: Every lower class keeps the bound in front of the display
- **WHEN** the display presents frames costing a quarter period, twice the period, and then stops, and a near, a background and an offline piece of three quarters of the bound ask in turn
- **THEN** each class is admitted with the same room, the bound past the display's next present; none has a quarter bound or a slack of its own
- **TEST** t2923 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A whole piece is queued behind the one that runs
- **WHEN** on a ring with a display that presents nothing a piece of the largest size runs, for the frame on screen's class, a near and a background class
- **THEN** a next piece of that size is admissible at once, and a claim of the whole bound is not
- **TEST** t2930 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: The preview's present waits without polling the device
`wait_presented` SHALL NOT poll the display device on a fixed short period: it sleeps on the frame's
work-done callback for the device time the pass timer measured for the frame (any submission on the
device fires the callback early), then checks, and waits again from a quarter of that time doubling, at
least a millisecond; the waits SHALL stop growing at the frame's own time, at least 4 ms
(`present_waits`). A blocking wait on the display device stays forbidden (the surface's present waits for
its lock).
Why: row 272 — the present was waited for by a poll every millisecond; found-defects 693 — with no cap the
doubling saw a frame the device ended late at up to twice its lateness.

#### Scenario: A present is waited for by a few checks
- **WHEN** the preview presents frames of a measured chain on a device and waits for each
- **THEN** each wait ends after the frame completed, with at most a few non-blocking checks (none per millisecond of device time)
- **TEST** t2260 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The present checks stop growing
- **WHEN** a frame was last measured at 0 (never), 0.3, 2 or 20 ms of device time
- **THEN** the first check comes after that time (at least a millisecond, 2 ms when never measured), and no later wait exceeds the frame's time or 4 ms, whichever is longer, which they reach
- **TEST** t2866 (`src/libs/kt-gpu/tests/own_waits.rs`)

### Requirement: The preview's stream memo is sized by the loop and the machine
The preview's stream memo SHALL start at 256 MB and, while a loop region is set over a stream stage,
SHALL want what that loop keeps — for every stream stage over the region, a reel's included, its own
positions there at its own picture (half the stage's RGBA frame at its size), plus an eighth, plus the
two pictures the next frame reads — plus each stage's checkpoint tier — two pictures every N positions
of the walk from the stage's chain anchor to its last position in the region — over the 256 MB working
set, and never above a quarter of the memory the process can still take, measured at run time when the
build, the loop or the quality changes (never at the moment on screen). The memory ledger SHALL grant that want after the kept composites'
(media-index), and the memo's bound SHALL be its base plus its grant; the pinned tier SHALL hold at most
half of the bound. The codec's per-frame working state (the clean encoder's) SHALL be evicted before the
pictures, past the newest few entries the next frame reads. With no loop, or with
the memory unknown, it SHALL be 256 MB; a smaller bound SHALL drop the least recently used entries at
once. The sizing SHALL be a trace record.

Why: found-defects 143 — at 1080 lines 256 MB held about 20 positions, so a longer loop recomputed its
codec history every lap; a constant fits one machine only. memory-ledger: the memo's quarter and the
present memo's growth were two budgets that did not see each other. found-defects 394: the walk from the
anchor to the loop was not counted, and was evicted and recomputed every lap. stream-memo-budget (rows 512, 518): the
want was every moment of the region at the size of the frame on screen and skipped reels — a short
Datamosh in a long loop over-asked History, which the ledger grants before Frames, and a stage of another
size re-sized the memo at each crossing.

#### Scenario: The bound follows the loop within the machine
- **WHEN** the bound is asked with no loop, with a loop needing less than the base, with one needing more on a machine with plenty and with little memory, and with the memory unknown
- **THEN** it is the base, the base plus the short loop's pictures, the base plus the loop's pictures, a quarter of the memory (not below the base), and the base; sizing the preview's memo sets its bound, drops what a smaller bound does not hold, and leaves a `stream_memo_cap` record
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: The want covers the walk's checkpoints
- **WHEN** the bound is asked for a loop of 40 positions whose chain anchor lies 400 positions before the region's end, on a machine with plenty of memory
- **THEN** it is the base plus the loop's pictures plus two pictures for every N of the 400 positions
- **TEST** t2355 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: A loop the sized memo holds computes nothing on its second lap
- **WHEN** a mosh is advanced over a loop of positions twice, with a memo sized for that loop and with one that holds half of it
- **THEN** the second lap of the sized memo runs no encoder or decoder job and gives the same frames; the half memo recomputes
- **TEST** t2276 (`src/plugins/fx-video-codec/tests/packed.rs`)

#### Scenario: The kept composites come before the history
- **WHEN** the stream memo is sized for a loop while the kept composites want most of the growth budget, then the composites' want drops
- **THEN** the memo's bound is its base plus what the composites left, never above its own want; when the composites want less, the memo is granted its want again at its next sizing
- **TEST** t2297 (`src/libs/kt-engine/tests/stream_memo.rs`)

#### Scenario: The want counts each stage's own positions, a reel's included
- **WHEN** a 6 s Datamosh sits at the root and a reel of it later, in a 60 s loop region otherwise plain, and the checkpoints of a walk are asked at the root and inside the reel
- **THEN** the want counts 2 × 180 positions at the stage's picture and both walks' checkpoint tiers, less than the region's 1801 moments at that size; a loop over the plain shot alone keeps nothing; the reel's walk names the same checkpoints as the root's
- **TEST** t2491 (`src/libs/kt-engine/tests/stream_memo.rs`), t2492 (`src/libs/kt-engine/tests/stream_memo.rs`)

### Requirement: A loop longer than the present memo keeps a part of it
When the present memo cannot hold the loop it was told of, a new composite SHALL NOT be admitted over
a kept one that the current lap has shown or will still show (one shown within the loop's length of
presents); it SHALL be admitted over one older than that, least recent first. Without a loop the memo
SHALL stay least recently shown first.
Why: an LRU over a cycle longer than itself evicts exactly the moment the next lap reads first, so a
loop one frame too long for the memo kept nothing; with this rule it keeps what fits, lap after lap.

#### Scenario: A loop three times the memo keeps a third
- **WHEN** a memo holding ten composites is told of a loop of thirty moments with no grant, and the loop is presented three times
- **THEN** the second and third laps each show ten moments from the kept composites and compose twenty; without the loop told, the second lap shows none from them
- **TEST** t2293 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: A frame decoded again finds its kept composite
A moment whose decoded frames were evicted from the burst cache and decoded again with the same bytes
SHALL be presented from its kept composite: the re-decoded entries take back their serials, so the
plan key is the one the composite was kept under.
Why: found-defects 119 — a frame evicted and decoded again missed its kept composite and was composed
again; the serial return of structured-telemetry (t2152) fixed the cache, and this holds it end to end.

#### Scenario: Evicted, decoded again, replayed
- **WHEN** a moment is presented and kept, its frame is pushed out of a small burst cache by other frames, the same frame is decoded into it again, and the moment is presented again
- **THEN** the second present is a replay of the kept composite: nothing is composed and the picture is the same
- **TEST** t2294 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: A device's refusal bounds the present memo
On a device whose textures are not the process's own memory as the ledger counts it, the present memo
SHALL create the texture its growth needs under an out-of-memory check before keeping a composite past
its base. A refusal SHALL keep the composite unkept, SHALL bound the memo's growth at three quarters of
what it holds then (dropping the rest at once, so the device has room for its frames), SHALL be a trace
record, and SHALL NOT poison the device or lose the frame on screen.
Why: the memo's grant is measured in the process's memory; a card's own memory is not readable through
the graphics API, so it is learned at run time from the device's first refusal, never set as a
constant for one card.

#### Scenario: A refused growth is learned
- **WHEN** a memo holding twenty composites past its base meets a refusal at the next keep
- **THEN** that composite is shown but not kept, the memo holds fifteen past its base, later keeps never pass that bound, the frame on screen is complete and the device is not poisoned
- **TEST** t2295 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: The executor binds and writes uniforms per frame, not per pass
The GPU executor SHALL write a frame's pass uniforms into its arena with one queue write per submitted
piece (a piece the arbiter cuts carries its own passes' uniforms before it is submitted), and SHALL reuse a
bind group when the same layout meets the same textures again, keeping at most a bounded set and never a
texture its owner released beyond the frame in which it was released. The frames SHALL be the same bytes
as with a write and a bind group per pass, preview and export, cut into pieces or not; after warm-up a
frame creates no GPU resource (t2023).
Why: found-defects row 138 (gpu-pass-cost): a bind group and a uniform write per pass on the composing
thread.

#### Scenario: Batched uniforms and reused bind groups keep the bytes
- **WHEN** a chain of geometry, point-wise runs, barriers (blur, glow, Camera ISP, VHS Tape) and a second layer are composed per frame and through the export ring (the arbiter's cut pieces: t2253, same bytes cut and uncut), with the per-pass form and the batched form
- **THEN** every frame is equal byte for byte between the forms
- **TEST** t2313 (`src/libs/kt-gpu/tests/local_cost.rs`)

#### Scenario: The steady state reuses its bind groups
- **WHEN** a heavy two-layer chain plays 24 moments (four in turn, each with its own uploaded frames) twice
- **THEN** the second lap reuses at least twice as many bind groups as it makes (measured 648 reused, 217 made: the misses are each moment's new uploads), and the cache never holds more than its bound
- **TEST** t2314 (`src/libs/kt-gpu/tests/local_cost.rs`)

### Requirement: A lower-class piece is admitted against the time in flight
On a bounded ring, a lower-class frame or compute job cut into pieces SHALL submit a piece and go on
encoding the next without waiting for the piece's completion; the next piece SHALL be admitted by the
ring against its predicted drain (the device time still ahead of each submission in flight, the display's
present included), waiting only until the drain has room or a higher class waits. Where a submitted piece
of a device without a surface is in flight, the waiter SHALL wait for the oldest one's completion and look
again the moment it comes; with none (a surface device's pieces, pieces still being encoded) it SHALL wake
when its claim fits by the prediction. A piece whose time is a guess (a pass never measured) SHALL make room
only by its completion, never by the clock. A ring with a display still never holds a piece of several
passes predicted over the bound in front of the display, and the cut frame's bytes equal the uncut frame's.
Why: found-defects 392 — `Piece::cut` waited for the device to finish each piece before admitting the
next: ~45 % of a 720-line Datamosh walk in his trace was the arbiter's own waits. Found-defects 480 — counted
by completions, a piece queued behind the display's frame held the ring for the whole frame. Stage 11's
finish.sh (t2435, t2436 red under the full parallel run, green alone): a guessed time drained by the clock,
so on a loaded machine the next piece went before the guessed one's timestamps came back and nothing was
learned; a waiter sleeping on a timer saw a completion only at its own next wake, and the device stood
idle between pieces. render-ahead (2026-11-17): the count of three lower pieces is gone with the half-frame
bound; the predicted drain alone bounds what stands in front of the display.

#### Scenario: A cut goes on while the bound has room
- **WHEN** a compute ring with no display and a ring with a display each take a lower-class job cut into pieces whose predicted times fit the bound together, and pieces that do not
- **THEN** the pieces that fit are admitted without waiting for the earlier piece's completion; the ones that do not wait until the drain has room; t2253 holds
- **TEST** t2356 (`src/libs/kt-gpu/src/arbiter.rs`), t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: A waiter is admitted when the predicted drain makes room
- **WHEN** a displayed ring holds a submitted lower piece predicted at the whole bound, never completing, and another lower piece of half the bound waits
- **THEN** the waiter is admitted once half the earlier piece's predicted time has passed, with no completion
- **TEST** t2431 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A guess makes room only by its completion
- **WHEN** a displayed ring holds a submitted lower piece whose time is a guess at the whole bound, never completing, and another lower piece of half the bound waits for twice that time
- **THEN** the waiter is not admitted by the clock; it is admitted when the earlier piece completes
- **TEST** t2441 (`src/libs/kt-gpu/src/arbiter.rs`); t2435 and t2436 (`src/libs/kt-gpu/tests/compute_start.rs`) hold under a full parallel run

### Requirement: The prefetch advances a run of stream frames as one walk
The prefetch thread SHALL advance a stream stage's frames ahead of the frame on screen as one walk to the
last frame of each run that does not wrap, not as one walk per frame; the walk reaches the frame on screen
first. The run has a NEAR part, while playing: the frames the display reaches before the next pass, twice
over, at class near; and a FAR part at class background: with the playhead inside a loop region, to the
region's end and round from its start, as far as the stream memo holds the positions; without one, while
playing, up to a quarter of the memo ahead. On a pause inside a loop region the far part runs too; on a pause
outside one, only the frame on screen. Both parts skip kept moments, stop at the first position whose source
is not decoded, and stop when the transport or the project leaves them. A far part left pending SHALL run
again on the next pass only while it walks or while the decoded-frame cache gained a video frame since the
pass before; one that walked nothing over a pass that decoded nothing waits for the pass state to move. The player SHALL tell the stream memo its view (the loop region's start while the playhead is inside
it, else the frame on screen), from which on a walk keeps every picture.
Why: found-defects 391 — one advance per frame made every position its own submission while playing.
Found-defects 482 and the owner's 11.11 L4 ("я не против чтобы оно наперед считало") — twice the display's
reach was ~100 positions a lap of his 715-position loop, six laps to finish it. render-ahead D6: the eight
retries of `FAR_STALLS` were a count of one machine; a decoded frame is what lets a stalled walk move.

#### Scenario: Frames ahead are one walk
- **WHEN** a mosh is advanced from the frame on screen through eight frames ahead
- **THEN** the history is computed by at most ⌈9 / batch⌉ + 1 jobs, and every one of the nine frames then needs nothing
- **TEST** t2360 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A loop is computed to its end, playing or paused
- **WHEN** a loop region over a mosh is set with every source frame cached, and the prefetch's advance runs once with the playhead at its start, paused and playing
- **THEN** the far part reaches the region's last frame in that pass (no frame of the region needs anything after it), ran at class background, and a pass with the history complete runs nothing
- **TEST** t2432 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A stalled far walk waits for a decoded frame
- **WHEN** a far walk is left pending after a pass that walked positions, one that walked none while the cache gained a frame, and one that walked none while the cache gained nothing; and video frames and an audio burst go into a cache
- **THEN** the first two run again on the next pass, the third stops until the state moves; the cache's count moves by every video frame and not by audio
- **TEST** t2924 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: No stage runs over a stand-in input
A composite SHALL NOT give any stage of the plan an input that is not that stage's own position — a
stream stage's picture of another position, a sub-composite of a reel or of a track stage that lacks a
layer, a neighbour of a frame pair, a decoded frame of another time — and run the stages after it (the
event's later effects such as VHS Tape or Camera ISP, the track's stages, the reel's stages) over it at
the moment asked. A layer whose own input is not there SHALL leave the frame incomplete; an incomplete
composite SHALL NOT be presented over the picture on screen and SHALL NOT be kept as the moment's
composite; the preview keeps its last full picture under its Loading look, and a preview that has none
keeps its place empty under Loading: no incomplete composite is ever presented, the first one included
(kkt-26.11.14 M1, which replaced round 14's "a picture under the scrim rather than an empty one"). A kept composite of
the exact moment SHALL still show at once, without the history. An export SHALL compose every frame from
its own positions, never short.
Why: kkt-26.11.10b K5 — "при кэтчинге - у меня vhs tape будто бы накладывается в этот момент
актуальный … либо есть кадр либо нет, зачем накладывать эффект если предыдущий не готов?": the
playing preview ran the event's VHS Tape at the moment asked over a Datamosh picture of a position
the history had reached.

#### Scenario: A playing frame whose history is behind is Loading, not a held picture
- **WHEN** a paced renderer composes a frame of a cut mosh whose history the prefetch has walked to ten positions behind it, then the history catches up
- **THEN** the first composite is incomplete, the mosh layer has no picture, the renderer reports at least eight positions short, and the executor presents nothing; after the catch-up the composite is complete, reports nothing short and is byte-identical to the unpaced renderer's
- **TEST** t2410 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: VHS Tape is never laid over another position's mosh
- **WHEN** the mosh event carries VHS Tape after Datamosh and its track carries VHS Tape as a track stage, and the frame is composed while the history is behind and again after it caught up
- **THEN** the short composite runs no chain of the mosh layer and is incomplete with nothing presented; the caught-up composite equals the unpaced renderer's bytes
- **TEST** t2411 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: An export composes every frame from its own positions
- **WHEN** a range of the mosh event with VHS Tape after it is exported through the serial export and through the pipeline
- **THEN** no frame is short, every frame's stream key is the stage's key of its own position, and the frames equal the unpaced renderer's at each moment
- **TEST** t2412 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A reel that holds a mosh reports its catch-up to the root
- **WHEN** the mosh project is placed as a reel and the root frame is composed through a paced renderer with the inner history behind
- **THEN** the root composite is incomplete and reports the inner positions short; after the inner history catches up the root composite is complete and reports nothing short
- **TEST** t2413 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The player answers a short frame with its catch-up and a kept moment at once
- **WHEN** a playing player is asked a frame of the mosh whose history is behind, then the same moment after its composite was kept and the memo dropped
- **THEN** the first answer is incomplete with a catch-up count above zero; the kept moment's answer is complete with a catch-up of zero, without the history
- **TEST** t2414 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A frame pair lacking a side is incomplete
- **WHEN** a slowed event in the frame mode "blend" is composed at a moment between two source frames with both frames cached, then with the neighbour missing, then with the base frame missing
- **THEN** the first is complete and blended; the other two are incomplete and their layer has no picture (neither side stands in at full weight)
- **TEST** t2418 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: An offline frame keeps only a complete composite
- **WHEN** the reel strip's and the poster's offline frame is composed with one layer's source absent, and a poster is made of a project whose file is missing
- **THEN** the incomplete frame leaves no picture (no partial cell); the poster of the project with the missing file is the frame with the placeholder, as the preview shows it
- **TEST** t2419 (`src/apps/kt-app/src/reel_preview.rs`)

### Requirement: The slow-chain frame never waits behind side work on its engine
Effect cells, reel strips and project posters SHALL compose on an engine of their own on the
offline device that shares its compiled pipelines but not the frame's state or lock; the slow-chain
route of the preview SHALL have the offline engine to itself. The offline device's ring SHALL bound
its flight once the slow route serves the preview from it: a lower-class frame submitted there is
cut at pass boundaries when the slow route's frame waits, so that frame waits for at most one piece.
Why: `found-defects.md` row 290 — the offline engine's lock was held for a whole frame, so the
slow-route frame waited behind an effect cell or a poster on the mutex, not on the ring that bounds
it; on a software device that ring did not bound either.

#### Scenario: A side frame holds the slow route for one piece
- **WHEN** a heavy chain composes on the side engine of the offline device (class background) and the slow-route engine asks a light frame at class now meanwhile, with the side engine's frame held mid-way
- **THEN** the light frame composes without waiting for the side engine's lock, its wait on the ring stays within one measured piece of the heavy chain, and both frames' bytes equal their solo runs
- **TEST** t2337 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The side engine compiles nothing
- **WHEN** the side engine is made after the offline engine
- **THEN** it shares the offline engine's compiled pipelines (no pipeline is created) and its own arena and pools
- **TEST** t2338 (`src/libs/kt-gpu/tests/arbiter.rs`)

### Requirement: A run prewarm warms the pieces a frame asks
A point-wise run handed to the executor's prewarm SHALL be warmed as the pieces a frame cuts it into
(at most `MAX_RUN_STAGES` stages each), so the first frame of a long run finds its passes warm.
Why: class-walk-revision — the prewarm passed the whole run as one key, and a key longer than the
cap is skipped; no frame ever asks it.

#### Scenario: A 40-stage run is prewarmed as 32 + 8
- **WHEN** a run of 40 point-wise stages is prewarmed and the device settles
- **THEN** two run shapes are warm: the 32-stage piece and the 8-stage tail
- **TEST** t2450 (`src/libs/kt-gpu/tests/fused_runs.rs`)

### Requirement: The stream memo's view belongs to the renderer that set it
The memo's view is a tick of the timeline of the renderer that set it (the player's preview
renderer). A stream stage SHALL apply it only in that renderer's own plan and its track stages; a
reel's sub-plan and any renderer sharing the memo that never set a view (a reel strip, a poster)
SHALL read every position of its walk back.
Why: class-walk-revision — the view was read by every renderer sharing the memo, so a walk on
another timeline read no picture back and every frame of it stayed short.

#### Scenario: A renderer that set no view reads its walk back
- **WHEN** two renderers share one memo, the player's sets its view at 8 s, and the other walks 5 s … 5 s + 8 frames
- **THEN** every frame of that walk needs nothing more
- **TEST** t2451 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: A reel strip raises generators and pauses its envelope
The reel strip SHALL open the generators' per-event sources as the poster does, so a cell a
generator shows has its picture; its envelope (the offline audio render) SHALL wait at each chunk
while its job is held, as its frames do.
Why: class-walk-revision — since only complete offline composites are kept (catch-up-shows-loading
D6) every generator cell was empty; the envelope checked only the cancel and ran during playback.

#### Scenario: A generator cell has its picture
- **WHEN** a reel's strip is rendered whose only event is a solid-colour generator
- **THEN** its first cell is the generator's colour
- **TEST** t2455 (`src/apps/kt-app/src/reel_preview.rs`)

#### Scenario: The envelope waits while held
- **WHEN** the strip job is held after the envelope's first chunk
- **THEN** no further chunk runs until it is released
- **TEST** t2454 (`src/apps/kt-app/src/reel_preview.rs`)

### Requirement: Caches size against the ledger's room
The preview's stream memo and the decoded-frame cache's loop count SHALL measure their want against
the memory ledger's room — the memory available plus the growth the growing kinds already hold —
never the bare available.
Why: class-walk-revision — the cache's own growth took the free memory, so a re-sizing asked less
than it held and the asker evicted what it had just kept.

#### Scenario: A re-sizing after the memo filled keeps its cap
- **WHEN** the memo is sized for a 400-position loop, fills to its cap, the machine has that much less available, and the same loop is sized again with one checkpoint more
- **THEN** the cap does not shrink
- **TEST** t2458 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: The tools name a frame behind its stream history
The preview bench SHALL report every incomplete composite whose stream history was behind with the
positions short; `kt-cli preview` and `kt-cli pause-scan` SHALL print it.
Why: class-walk-revision — such a frame misses no decode, so the tools' one cause named nothing.

#### Scenario: A cold start over a mosh is named catching up
- **WHEN** the bench plays a range of a Datamosh event from a cold start
- **THEN** its incomplete composites are reported catching up, each with a positive count
- **TEST** t2459 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: The display's present is counted on its ring
The display's present (a composite, or a kept composite's display pass) SHALL enter its ring's flight as a
`now` submission predicted at its measured device time, without waiting for admission, and leave it when the
device completes it; the ring SHALL know when the display last submitted.
Why: found-defects 480 — the display's 32.5 ms composite on his card was invisible to the ring.

#### Scenario: The display is counted, never admitted
- **WHEN** a lower piece holds the bound and the display presents, then the display presents behind its frames
- **THEN** the present enters the flight with no wait; a lower class may queue what drains before the display's next present plus one frame period and not more; behind a late display frame a lower piece waits only for the bound past that frame's end
- **TEST** t2433 (`src/libs/kt-gpu/src/arbiter.rs`)

### Requirement: A pass never measured is predicted
A render pass or a compute dispatch SHALL be predicted from the device time measured for it on this device:
in this process, or — for one never measured in it — from a table kept on disk keyed by the adapter's name
and driver, the pass's label and its work size (scaled linearly from the nearest measured size of the same
label). A compute job SHALL learn its dispatches' times from its own first pieces while it runs. The table is
a cache: it changes where frames are cut, never their bytes.
Why: found-defects 481 — a pass never measured claimed the whole bound, so the first walk was one dispatch a
piece (267 in 267).

#### Scenario: A new process predicts what an earlier one measured
- **WHEN** pass times are noted for an adapter, saved, and a fresh table loads the file; a size never measured is asked, and another adapter asks the same label
- **THEN** the fresh table predicts the noted times, the unmeasured size from the nearest measured one scaled by work, and nothing for the other adapter
- **TEST** t2434 (`src/libs/kt-gpu/src/seeds.rs`)

#### Scenario: A cold walk is cut by what its first pieces measured
- **WHEN** a compute job of many dispatches of four names, never measured, runs on a bounded ring, then the same job again
- **THEN** the first run cuts before an unmeasured dispatch at most once a name (the cuts its measured times call for follow the machine's speed), and its bytes equal the second's
- **TEST** t2435 (`src/libs/kt-gpu/tests/compute_start.rs`)

### Requirement: Work a person waits for uses the machine
Work a person waits for — the catch-up of a stream history, the Loading of a paused frame, a loop's first
laps, a render or export, thumbnails and strips he scrolls to, an analysis he started — SHALL use the idle
device and the idle cores within the limits the owner set: the display first (the ring's bound), and rest
costs nothing (no work at rest, invariant 12). A path that can split its work without changing a byte SHALL
split it across cores; a CPU executor SHALL take a device's work only where its bytes equal the device's,
or by a choice that does not follow the load (invariant 11). It is measured from pass timestamps and thread
time, not wall-clock guesses.
Why: found-defects 485 — the owner, kkt-26.11.10b K3: "не грузит проц - ленится как будто опять или
упирается в одно ядро иногда, у нас нет спек на эту тему??"; 11.11 L4: "и чтобы и процессор тоже
использовался, для тех же вычислений".

#### Scenario: A walk keeps the device fed
- **WHEN** a cold Datamosh walk of many positions runs at class near on a compute ring that carries a display with no display traffic
- **THEN** from the trace's per-piece submits, device starts and ends, at least half the pieces are submitted before the earlier one ends, and the device stands idle with nothing queued (after a piece whose successor was not yet submitted) for less than a fifth of the walk's device span; idle behind a piece already queued is a software device's queue thread waiting for the processor and is reported, not held
- **TEST** t2436 (`src/libs/kt-gpu/tests/compute_start.rs`, run with the machine: on a software device the queued-ahead count races the device's threads)

### Requirement: A loop longer than the stream memo keeps a part of it
Told of a loop (`keep_loop`), the stream memo SHALL NOT evict an entry the current or the last lap used
for a new one while it holds anything older; a new entry put into a memo full of the loop's laps SHALL
enter a window of at most a quarter of the bound past the kept part, oldest out first, so the
composite still reads what the look-ahead just computed. The host SHALL mark a lap when its loop comes
round (`lap`); a new loop SHALL start a fresh count; without a loop the memo SHALL stay least recently
used first.
Why: stream-memo-budget (row 517) — an LRU over a cycle longer than itself evicts exactly the position
the next lap reads first, so a loop History could not hold recomputed every position every lap; the
present memo kept a part (memory-ledger D5), the stream memo did not.

#### Scenario: A loop three times the memo keeps a part
- **WHEN** a memo holding ten positions is run over a loop of thirty for several laps, each computed position read back at once, with the loop told and without; then over a loop it holds; then over a new loop after an old one
- **THEN** without the loop no lap finds anything; with it every lap after the first finds at least seven and the same number lap after lap; a held loop is found whole; the new loop keeps its own part
- **TEST** t2490 (`src/libs/kt-plugin-api/src/stream.rs`)

### Requirement: The compute host's every submission is admitted and a job binds by offset
The compute host SHALL admit every submission on its ring by the caller's class: a job's pieces, its clock
calibration (taken after the admission, so a wait never widens its bracket; a job finding another
calibrating goes on without, so a frame's job never waits behind a lower class's calibration) and a
shader's warm-up dispatch. A job SHALL write every step's uniform into one arena with one queue write and
bind each step's slot by a dynamic offset, with one bind group for every step of the same shader, slot size
and buffers within the job.
Why: found-defects 511 (the calibration submitted outside the ring) and 520 (a uniform buffer, a write and a
bind group per dispatch: ~2.4 ms of a 267-dispatch Datamosh walk).

#### Scenario: The calibration is admitted
- **WHEN** a traced job is the compute host's first, on a device with timestamps
- **THEN** the ring admitted the job's pieces and the calibration's three submissions
- **TEST** t2510 (`src/libs/kt-gpu/tests/compute_arena.rs`)

#### Scenario: A job binds its uniforms by offset
- **WHEN** a job of 240 dispatches over two shaders, each with its own uniform, adds its uniform into a buffer
- **THEN** the buffer holds the exact sum, and the job made two bind groups
- **TEST** t2511 (`src/libs/kt-gpu/tests/compute_arena.rs`)

### Requirement: The readback memo keeps complete composites and finds them without a scan
The executor's readback memo SHALL keep only complete composites, and SHALL find, bump and evict an entry
without scanning its keys (a generation per entry).
Why: found-defects 523 — incomplete composites, keyed by what they lacked and never asked again, pushed
complete ones out of the 64 MB the strip, poster, picker and slow route share; 521 — the LRU was a linear
scan per frame.

#### Scenario: The least recently asked goes first
- **WHEN** three frames fill the memo, the first is asked, and a fourth is kept
- **THEN** the second is evicted, the first stays; a key kept again replaces itself in place
- **TEST** t2512 (`src/libs/kt-gpu/src/pool.rs`)

#### Scenario: An incomplete composite is not kept
- **WHEN** two complete and ten incomplete composites are kept into a memo of two
- **THEN** the complete ones stay and no incomplete one is kept
- **TEST** t2513 (`src/libs/kt-gpu/src/pool.rs`)

### Requirement: The look-ahead walks only what a stage on screen reaches
The far look-ahead of a stream history SHALL walk nothing while no stream stage is on screen, and no
look-ahead walk SHALL take more than `STREAM_WALK_SECS` of frames. The walk of the frame ON SCREEN SHALL
run at the display's class; the frames after it at the look-ahead's.
Why: found-defects 660, 662, kkt-26.11.15 N2 — with a stage frame size of 0 the far walk was 33.5 million
frames, 26.5 s of the prefetch thread every pass, and a piece without Datamosh played at 8.7 V-fps
against kt26.11.10's 21.6; the frame on screen's history waited behind every present at the look-ahead's
class.

#### Scenario: No stage on screen walks nothing far
- **WHEN** a piece with no stream stage is on screen in a project with a Datamosh elsewhere, playing
- **THEN** the far walk is 0 frames, and inside the stage it is at most a minute of frames
- **TEST** t2710 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The frame on screen walks at the display's class
- **WHEN** the look-ahead at `near` walks a cold history to the frame on screen and eight frames after it
- **THEN** the jobs up to the frame on screen run at `now` and the rest at `near`
- **TEST** t2711 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: The slow route's picture is shown from the display's memo
A complete composite of the slow-chain device, once kept on the display's executor, SHALL be shown from
there at once, not handed to the interface as bytes to upload again.
Why: found-defects 622 — the slow route's frame was written into the present memo and uploaded again into
the interface's texture.

#### Scenario: The slow route shows its kept picture
- **WHEN** a moment of a chain slower than `SLOW_CHAIN` is composed on the slow device
- **THEN** it comes back on the display, the next ask is a replay with no slow run, and a moment not kept runs on the slow device and is shown on the display
- **TEST** t2705 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: A frame's work grows with the events active at it
Every per-frame question of a video renderer — the plan, the order (`layers_at`, `wanted_frames`,
`absent_layers`), a stream stage's advance, want, bytes and checkpoints — SHALL find the events active
at the moment through the renderer's time index (`LayerIndex`), laid out once per build in the
composite order; a dissolve's partner and a track stage's events, revision and masks SHALL be laid out
once per build too. The index SHALL answer exactly the former scan, in its order. A stream stage's
second input at a position SHALL be found by halving, not by a scan of the lane.

#### Scenario: Far events do not tax a frame
- **WHEN** a scene with a dissolve, an animated layer, an animated mask and a camera is planned over 50 frames with 20, 80 and 320 one-second events far after it
- **THEN** the events looked at (`Cost::FrameClip`) are the same count in all three
- **TEST** t2660 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: The order costs its window
- **WHEN** `wanted_frames` of the scene's first two seconds is asked with 20, 80 and 320 far events
- **THEN** the count is the same in all three
- **TEST** t2661 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: The index is the scan
- **WHEN** the events active at every 1/50 s of a scene with overlaps, transition handles and an event under everything are asked of the index and of the former scan
- **THEN** both give the same events in the same order
- **TEST** t2662 (`src/libs/kt-engine/tests/frame_cost.rs`)

### Requirement: A frame's own GPU error is its frame's error
Every engine entry point that composes (`execute_measured`, `execute_into`, `execute_once`,
`execute_held`, `present`, `present_kept`, `submit`, `collect`) SHALL run inside thread-local
Validation and Internal error scopes of its device; an error raised there SHALL return
`GpuExecError::Validation` (poisoned until the end of the session in the preview, an error in an
export; `present_kept` shows nothing) and SHALL NOT reach the device's uncaptured-error handler.
Why: kkt-26.11.15 N1 — no frame ran inside a scope, so any wgpu error a frame raised ended the
program through wgpu's default handler instead of the poisoning contract the player has.

#### Scenario: A frame whose upload overruns returns its error
- **WHEN** a frame's layer says 64×64 and carries the bytes of 4×4
- **THEN** `execute_into` returns `GpuExecError::Validation` with the cause, the uncaptured count does not move, and the next frame of 16×8 composes (before: wgpu's default handler panicked inside the frame)
- **TEST** t2721 (`src/libs/kt-gpu/tests/gpu_errors.rs`)

### Requirement: The preview's executor on GL is off the window's device
Where the window's device is the UI thread's alone (GL), the preview's executor SHALL be an engine on a
headless GL device of the process (`preview_engine_off_display`: the device and its pipelines made
once, a `sibling` engine per project) and the preview SHALL take the bytes path; the geometry dump SHALL
say so (`compose.ui_only`). Elsewhere the display path stays.
Why: on GL every wgpu call holds the adapter's one context lock, and the display engine's compose,
build and warm work on the window's device made egui's paint wait for it (gl-context-holder).

#### Scenario: A kept moment is read back, not composed
- **WHEN** the preview's engine off the window's device composes a moment, then is asked the same moment again
- **THEN** the first is kept (the coverage bars see it), the second is a replay with nothing composed, and both carry the same bytes
- **TEST** t2746 (`src/libs/kt-gpu/tests/gl_context_holder.rs`)

#### Scenario: Video FX on GL show their bytes
- **PATH** open two flat light-gray clips, the second with Edges → click the first: the preview is light → click the second: the preview is black → the dump says the preview composed off the window's device on GL (on another backend: the picture stays on the device)
- **TEST** t2744 live scenario `src/scripts/live/gpufx.py`

### Requirement: Every wait for a device names its own submission
A thread that waits for what it submitted SHALL wait for that submission's index: a read-back frame, a
pipeline warm-up draw, a compute job, the compute host's clock calibration and its shader warm-up. Waiting
for everything submitted to the device SHALL be left to what that is the job of (a parked engine,
`finish_present` for tests and benches), and those waits are counted (`kt_gpu::waits_for_all`).
Why: found-defects 693, 661 — a wait for everything also waits for whatever other threads submit to the
device meanwhile (the loop fill's eight lanes on one device, a look-ahead's pieces), and holds the device's
lock through it, against the window's present on the display's device; the calibration's bracket, the clock
error of every compute record, widened to 39 ms on the owner's iGPU.

#### Scenario: No frame or job waits for everything
- **WHEN** an engine warms its pipelines and reads back three frames, and the compute host runs a job
- **THEN** no wait for everything submitted to a device was made
- **TEST** t2864 (`src/libs/kt-gpu/tests/own_waits.rs`)

### Requirement: A ring's waiter sees a completion on any device in flight
A waiter of a ring whose flights are on more than one device SHALL look at every one of those devices at
least every 2 ms while it waits; it SHALL NOT block on one device's oldest piece until that piece ends.
Why: found-defects 662 — the devices of one card finish out of each other's order; a waiter blocked on the
oldest piece of one device missed the room a piece on another made, and near work waited while the card idled.

#### Scenario: A waiter sees a completion on another device
- **WHEN** a ring with a display holds a long piece on one device, a short piece on another, and the rest of the lower pieces it keeps uncompleted, and a near piece comes to be admitted
- **THEN** it is admitted once the short piece ends, while the long one still runs
- **TEST** t2865 (`src/libs/kt-gpu/tests/own_waits.rs`)

### Requirement: Playing, the display presents finished frames and one producer renders ahead
While the transport plays and the player's render-ahead producer has an executor, the preview's display
SHALL only present composites the display's executor already keeps: a moment asked that is not kept SHALL
give no picture (the preview keeps its last full picture under Loading), run no plan, ask no decode and
wake the producer. The on-demand composite SHALL remain for a paused transport, scrubbing and seeking, for
an ask without a display target (the bytes path), and for a player whose producer has no executor.
`KT_RENDER_AHEAD=off` turns the producer off and restores the on-demand composite while playing (an A/B of
one binary).

The producer SHALL be one `near` service of the scheduler. Each pass SHALL look from the frame on screen
plus its measured lead (the wall a moment takes, at least one frame) forward: inside a loop region round
the region (every moment of the lap); outside one forward until the first moment whose layers are not all
decoded. It SHALL skip moments the display's executor holds (noting them for the coverage bars) and moments
skipped this lap — one the display's memo did not keep. A moment that came out incomplete SHALL wait for
what it lacked, not for the lap's end: one whose stream stage was short of `k` positions until the history
walks computed `k` less the positions a composite computes itself, any other until a walk or a decode
moved; a new lap tries it again. A moment skipped or waiting is composed on demand by the display. The
producer SHALL compose the rest in that order, keeping each complete composite on the
display's executor under its plan key. It SHALL ask no decode and withdraw no request. Nothing SHALL run
while nothing plays.

The producer SHALL compose on an engine of the display's own device (its own lock), on a card and on a
software rasterizer alike, and the composite SHALL reach the display's present memo on the device, with no
readback, no upload and no wait for the device; the producer learns that the device finished a moment from a
done flag the device sets. Only where the display's executor has no engine of its own device (or
`KT_RENDER_AHEAD_DEVICE=side`) it SHALL compose on the offline device's side engine and hand the bytes to the
display's executor. The moments in flight at once (the depth) SHALL be measured, never a constant of
one machine: the wall a moment takes from its plan to its completion over the producer's own time per
moment, at least one, at most the moments whose bytes the memory ledger has room for. A kept composite's
bytes SHALL equal the bytes the display's own `present` would have kept for the same plan.
Why: `docs/architecture-review-26.11.17.md` §1–§2, R2 — on the owner's iGPU the loop fill's moment took ~174
ms of wall for ~12 ms of device, the card was 4–26 % busy while frames were missing, and a VHS Tape loop the
card could compose in real time took seven laps; the owner raised the bound and asked for computing ahead
(kkt-26.11.17 S4, kkt-26.11.11 L4). Found-defects 251 — a display slower than the project's frames kept only
the moments it showed. kkt-26.11.18 T2 — a moment short of its history was skipped for the lap, so a moment the walk reached
a second later waited for the next lap: of 300 moments composed ahead a lap, 6–58 were kept, and his loop
filled from lap 15.

#### Scenario: A loop shown in part becomes all-kept
- **WHEN** a loop of 24 moments plays with its frames decoded and a display that presents only every fourth moment it reaches, for three laps, with a producer executor and with `KT_RENDER_AHEAD=off`
- **THEN** with the producer every moment of the loop is kept by the display's executor by the third lap and the display's answers of the last lap are all kept composites; without it a part stays unkept
- **TEST** t2334 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The producer asks no decode and rests with the player
- **WHEN** a loop plays with half its frames not decoded, then the play stops
- **THEN** the producer composes only moments whose frames are decoded, the workers received no request from it, and after the stop it runs no job
- **TEST** t2335 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The producer skips what the memo does not keep
- **WHEN** a loop plays whose moments the display's memo refuses past the first eight
- **THEN** the producer keeps eight, skips the rest for the lap, and composes nothing more until the next lap
- **TEST** t2336 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: On a card several moments are in flight on one engine
- **WHEN** a loop of 24 moments plays with a producer executor that reports a hardware device, returns from each moment at once and finishes it 40 ms later (a device's done flag)
- **THEN** at least two moments are in flight at once, no moment is composed twice in a lap, and at least half of the device latency the same executor pays when every moment is waited for is hidden
- **TEST** t2920 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: The depth comes from measurement and memory
- **WHEN** the depth is asked for a moment's wall and the producer's own time per moment, with and without a memory room
- **THEN** it is the wall over the own time rounded up, at least one, one for a synchronous moment, and never more moments than the room holds
- **TEST** t2921 (`src/libs/kt-engine/src/render_ahead.rs`)

#### Scenario: A moment kept on the device is the display's own composite
- **WHEN** a heavy chain's plan is kept on a display engine by a sibling of it (on the device), by another engine (bytes), and presented by the display engine itself
- **THEN** the sibling's path made no readback, all three keep the same key, and the kept composites read back equal bytes
- **TEST** t2922 (`src/libs/kt-gpu/tests/render_ahead.rs`)

#### Scenario: Playing, the display shows only kept moments
- **WHEN** a player with a producer executor plays, and the display asks for a moment not kept and then for one kept; then the player pauses and asks the first moment again
- **THEN** playing, the first answer has no picture and ran no plan on the display's executor, the second is the kept composite; paused, the moment is composed on demand
- **TEST** t2925 (`src/libs/kt-engine/tests/render_ahead.rs`)

#### Scenario: Measured on a software rasterizer
- **WHEN** `kt-cli preview <showcase> 70 74 --size 640x360 --loop 40` runs on llvmpipe (release) on the stage-18 base and on this change, each recorded with `--trace`, and `kt-cli trace loop` reads each run
- **THEN** both runs and the device's busy share while frames were missing are in the change's design record
- **TEST** bench `kt-cli preview --loop` + `kt-cli trace loop`

#### Scenario: Loop playback in the program
- **WHEN** the live scenario opens a project with a heavy effect, sets a loop region and plays it for several laps
- **THEN** the preview shows the loop's frames (no lasting Loading after the first laps), the coverage bar over the region reads kept, and the process rests after the stop
- **TEST** t2927 (`src/scripts/live/render_ahead.py`)
- **PATH** open the project → drag over the ruler above the first event to set a loop region → press Space → the preview plays the region round and round; after the first laps the region's bar turns bright → press Space → the preview stands on its last frame

#### Scenario: An incomplete moment waits for what it lacked
- **WHEN** a moment comes out short of its stream history by 64 positions with a composite computing 2 itself, and another short by 2, and one with no stream stage
- **THEN** the first is not tried again until 62 positions were walked, the second as soon as a walk or a decode moved, the third as soon as a decode moved
- **TEST** t2931 (`src/libs/kt-engine/src/render_ahead.rs`)

### Requirement: Slow moments are prerendered at rest as hardware-encoded video and played from it
The preview SHALL keep complete composites of slow moments on disk under a DURABLE key: the plan key's
inputs (every field the executor's plan key hashes) with each frame's per-process content serial replaced
by what determines the frame across processes — a file's path, size and modification time, a generator's
values (`src_rev`), a stream stage's own key plus the identities of the project's files — salted by the
format, the program build and the device, so the same plan gives the same key in another process and an
edit, a replaced file, another build or another device gives another. The key SHALL NOT depend on the
codec, the encoder or the container: it names the picture, and a later reader (the cached final render,
F-EXP-003) can match it. The pictures SHALL be stored as video segments — runs of consecutive moments, every
frame a keyframe — written in-process by the machine's HARDWARE video encoder (the export's encoder cascade
with its real open check; H.264 at a high quality) and read back by our libav decoder; each segment's
manifest SHALL name the codec, the encoder, the frame size and rate and the durable key of every frame. No
processor encoder SHALL be used unless the host forces one by name (`KT_RENDER_CACHE_ENCODER`: tests and the
live check on a machine without a hardware encoder); where no hardware encoder opens the store writes
nothing and the Diagnostics window and the trace say why. The total SHALL be bounded (2 GiB unless
`KT_RENDER_CACHE_MB`), evicted a segment at a time, least recently used; the folders of other builds SHALL
be removed when the store opens. A restored picture is the composite within the codec's tolerance.

Before composing a moment the display's executor does not hold — the render-ahead producer before it asks
whether the moment is decoded, the compose thread before it composes — the preview SHALL look the moment up
on disk and, found, hand it to the display's executor and show or note it, asking no decode and running no
stage or chain. The coverage bars SHALL draw the moments the store holds for the current state.

The player SHALL prerender by itself at rest: when it is not playing, no export runs, and no input came for
1.5 s, it SHALL compose the moments whose measured device time is at least half a frame period and the
unmeasured neighbours of such moments (inside the loop region first, then nearest the playhead), its
device work at class `background`, its decodes at the lowest priority of the player's own workers, and
write those measured heavy. Any input, Play, an export or the player's close SHALL stop it before its next
step and abandon the moment. With no store set, nothing is read or written and the preview is as before.
No export SHALL read or write the store.
Why: kkt-26.11.17 S5 — the owner chose a prerender of slow regions "Сам в фоне" (review R3), his one
exception to "the program at rest costs nothing"; a heavy loop was composed again on every restart and
whenever the display's memo lost it. kkt-26.11.18b U4 — the storage: not raw, not a processor codec ("слабо
я верю что ffv1 аппаратно ускорен"), a normal video codec on the hardware ("сделай ты сжатие видео
нормальное"); lossy accepted, the export never reads it. U5 — the key is reused later by the cached final
render.

#### Scenario: The store keeps segments within its cap
- **WHEN** segments of two frames are written to a store with a small cap, two of them read, another build's folder exists at open, a segment's video is truncated, and a codec opens no encoder
- **THEN** each frame reads back as written, the manifest names the codec, the encoder, the size, the rate and the keys and is its own text, the least recently used segment goes first, the other build's folder is gone, the truncated segment is a miss and removed, the prerender evicts nothing this process used, and the codec with no encoder refuses the segment
- **TEST** t2980 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: A segment is written by the hardware encoder or refused
- **WHEN** a segment is opened with the default codec on a machine, and again forced to the export's software H.264, then six frames are written and read back out of order
- **THEN** the default opens a hardware encoder or refuses naming why, leaving no file; the forced segment names its encoder and every frame reads back alone within the codec's tolerance
- **TEST** t2986 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: A segment is read by the hardware decoder, the processor only without one
- **WHEN** a segment is read on this machine (no render node here), and in a child process on the fake VA driver of `fixtures/fake_va` (its encoder writes I_PCM macroblocks, its VLD entry point reads them back) a segment its encoder wrote is read out of order and twice
- **THEN** on this machine the reader names the processor and why (no VAAPI device) and reads every frame as the processor's decoder does; on the fake device every decoder of the file decodes on it (`vaapi <node>`, also the ones a later read adds), each frame is within the RGB → 4:2:0 → RGB round trip of the frame written (mean ≤ 3) and within swscale's rounding of NV12 against yuv420p of the processor's decode (mean ≤ 2), and the driver counts at least the pictures read; the disk tier's reader records which decoder reads (`render_cache` aux `READER`). Windows asks D3D11VA the same way (found-defects 775)
- **TEST** t3129 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: The durable key survives a process and follows what the picture is made of
- **WHEN** a moment's durable key is computed by two renderers over two frames caches (different serials), with its frames decoded and not; then after an edit of an effect parameter, a file's modification time, a generator's value, the quality and the build salt
- **THEN** the first keys are equal; each change gives another key
- **TEST** t2981 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: A prerendered moment plays from disk after a restart
- **PATH** play a slow part with VHS Tape and stop → after a second and a half with the mouse still the bars over the part turn bright → close and open the project, play the part → it plays from the first lap
- **WHEN** a player measures a heavy moment, rests until the prerender wrote it, and a new player over the same store and project plays that moment
- **THEN** the second player's display shows the moment from disk on its first ask while playing, its workers received no decode request for it, and with a lossless test codec the restored composite equals the bytes the producer's engine composes for the plan
- **TEST** t2982 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The prerender picks what was measured slow
- **WHEN** the cost map holds heavy moments (at least half a frame period), light ones and unmeasured ones next to the heavy, and a loop region
- **THEN** the candidates are the heavy moments not on disk and their unmeasured neighbours, the loop region's first, then nearest the playhead; a light moment and the moments past it are not candidates
- **TEST** t2983 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: The prerender yields to the person
- **WHEN** heavy moments are measured and the player stands paused while input keeps coming; then the input stops; then Play is pressed while it works; then an export vote is up
- **THEN** nothing is composed while input came within 1.5 s; after the quiet it writes; Play abandons the moment in work and nothing is written while playing; nothing runs under the export vote
- **TEST** t2984 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The bars draw what the disk holds for this state
- **WHEN** the store holds a prerendered loop and a new player opens the project with the loop region set; then an effect of the event is edited
- **THEN** after the scan at rest the bars draw the loop as playing without work, with no device work and no decode; after the edit they do not
- **TEST** t2985 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The editor prerenders a slow loop by itself and plays it from disk after a restart
- **PATH** open the heavy project with a one-second loop → Play: the loop plays slowly → Pause and leave the mouse: the program writes the loop's moments to its render cache, then rests → close and open the project again, Play → the loop plays from the disk from its first lap
- **WHEN** the editor plays a loop under Slowed, VHS Tape and Camera ISP on a software rasterizer, pauses with no input, and is started again on the same project (the encoder forced to the export's software H.264: the virtual screen has no hardware encoder)
- **THEN** segments appear in the render cache at rest and the program then rests; the first run's trace records heavy prerendered moments and the encoder; the restart's trace counts moments restored from disk in its first lap
- **TEST** t2988 (`src/scripts/live/render_cache.py`)

### Requirement: An offline job takes the moment the preview already holds
A background job that wants a picture of a moment of the current project version — the project's poster
— SHALL first ask the player for what the preview holds for it (`Player::kept_picture_reader`): the
composite the display's executor keeps under the moment's plan key, read back off the UI thread, or the
disk tier's picture of it; only when neither exists, or the build is not the version asked, SHALL it
compose the moment itself.

Why: found-defects 767 — after every Ctrl+S the poster walked a sharp Datamosh's history again, 22.6 s of
a software device, for the moment the preview had just shown (poster-from-kept D1).

#### Scenario: A kept moment is read back without its history
- **WHEN** the preview has presented a moment over a cut mosh, the stream memo and the decoded frames are dropped, and the reader is run for that moment and for one never composed
- **THEN** the first gives the picture at the preview's size with no stream run, walk or decode request; the second gives nothing
- **TEST** t2978 (`src/libs/kt-engine/tests/decode_storm.rs`)
