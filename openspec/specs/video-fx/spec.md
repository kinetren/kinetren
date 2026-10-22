# Video effects

## Purpose

A registry of 27 kinds of video effects (point-wise, barriers, geometry, 3D), a single
chain planner with a CPU reference and a GPU mirror, and the picker gallery with
thumbnails, variants and the strength axis — the picture in a cell equals the result of a click.

## Requirements

### Requirement: The video effect registry is closed by a golden cycle
Every video kind from the shared registry `FxInstance::all()` SHALL have a row in the
golden table of default templates, and the table SHALL cover the registry in both directions:
a new kind does not build until its row is added. Every kind MUST declare its
class to the planner (point-wise / barrier / geo) — an undeclared kind is
caught by a panic when run through the barrier path. The default of any new
parameter MUST give the former behaviour byte for byte: the goldens are not
regenerated when parameters are added. The defaults of "identity" kinds
(Geometry, Pan 3D, Color Grade, Corner Pin, Drop Shadow on an opaque frame)
legitimately share one identity hash; the kind `Pan` was removed from the registry, and it has no row in the table.
Why: "a new variant compiles and silently does nothing" — a forgotten arm of the
classification became a barrier and broke geo series; the completeness lock makes this
a build error, not a discovery of the user.

#### Scenario: A new kind will not build without a row
- **WHEN** a video kind is added to the registry without a row in the golden table or with a forgotten class
- **THEN** the golden loop fails on the completeness lock (or on the debug panic of the barrier path), and the hashes of all former defaults stand
- **TEST** t328 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: The ten original kernels and their properties
- **WHEN** each of the kernels of the first ten is applied to a test frame
- **THEN** the hashes match the reference; the mirror is an involution, blur preserves the mean and reduces the variance, greenscreen zeroes the alpha of the key corner, noise is deterministic by position, grid tiles are identical, the leading "Geometry" node with defaults is identity
- **TEST** t26 (`src/libs/kt-fx/tests/video_fx.rs`), t26 (`src/libs/kt-engine/tests/reverse.rs`) — the chain is applied in the single composite path, the cache stores the pre-FX frames (capability `video-plan`)

#### Scenario: A new parameter changes the picture, the default is identity
- **WHEN** an effect has gained a parameter (radius_y, thickness, noise size/colour, glow threshold/tint, VHS scan_step/jitter, key spill)
- **THEN** each non-zero value meaningfully changes the frame, and the default gives the bytes of the former kernel; the check runs over ALL channels, including those the kernel previously did not touch
- **TEST** t228 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: A grid larger than the frame does not overflow
- **WHEN** EventGrid is asked for more columns/rows than the frame has pixels
- **THEN** the grid is drawn from zero without centring, the frame size is integral, the result is deterministic and mirrors the GPU guard
- **TEST** t299 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: The buffer pool does not change the bytes
- **WHEN** the chain is executed with a "dirty" reused buffer pool
- **THEN** the result is byte-identical to a fresh run for each kind and for the chain Mirror+Grid+Glow+VHS
- **TEST** t68 (`src/libs/kt-fx/tests/video_fx.rs`)

### Requirement: CPU is the byte reference, GPU is a mirror within tolerances, edited as a pair
The CPU kernels (`video.rs`, `geom.rs`, `plan.rs`) SHALL be the reference of determinism
and the full fallback, including 3D. The GPU mirrors SHALL be checked against the CPU only through
the shared tolerance helper with tolerance classes (EXACT / BILINEAR / THRESHOLD /
GEOM); an ad-hoc epsilon MUST NOT appear. An edit of the formula of a kernel, of the planner
or of a geo primitive MUST be accompanied by an edit of the WGSL mirror (including
`geom_fused.wgsl`) in the same commit. The SIMD kernel MUST be byte-for-byte equal to the
scalar one; the scalar is the definition of the bytes and is not refactored together with the kernel.
Why: two executors of one formula silently diverge; the conformance loop over
the registry and the paired edit are the only way to keep them together.

#### Scenario: Conformance per effect and per chain
- **WHEN** each kind, geo series, pan3d chains, tap ops and K× barriers are computed on GPU and CPU
- **THEN** the divergence fits within the tolerance class of the kind; geo series and Pan3d — within GEOM
- **TEST** t181 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The conformance loop over the whole registry
- **WHEN** each video kind is run alone and "sandwiched" [grid, effect, zoom ×2], plus configured instances of identity defaults and warp-hard cases
- **THEN** the tolerance class is set by a table with a completeness lock; a failure of warp-hard — investigation and introduction of WARP from a measurement, not tuning GEOM
- **TEST** t329 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The AVX2 kernel equals the scalar one
- **WHEN** bilinear, noise, glow, edges, VHS and round-half-away are executed by the AVX2 path on non-multiple sizes and extreme values
- **THEN** the bytes match the scalar implementation
- **TEST** t198 (`src/libs/kt-fx/tests/simd_parity.rs`)

### Requirement: The chain plan is built by one code
All decisions about the chain — segmentation into geo runs and barriers, routing of a
single step to the old kernel or to the fused pass, absorption of point-wise ops into runs,
the multiplicity K of barriers, the snapshot points of the layer's strength — SHALL be made by one
planning function in kt-fx; the CPU and GPU executors MUST execute the finished
plan and decide nothing themselves. The plan of a slice between the strength stops of a layer SHALL be a
piece of the plan of the whole chain. Audio effects do not get into the plan and do not
break a geo series.
Why: before the single planner, the chain traversal lived in two copies (CPU and GPU) and
diverged; now a divergence of segmentation is impossible by construction.

#### Scenario: Segmentation and budgets
- **WHEN** a chain contains geo steps, barriers, layer strength markers and exceeds the budget of primitives
- **THEN** the plan consists of runs/barriers/snapshots/mixes, the window is squeezed along the geo boundary, and the plan of a slice equals a piece of the plan of the whole
- **TEST** t306 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: A series of length 1 — the old kernel, byte for byte
- **WHEN** a single Pan/CamMotion/Mirror without minification; a series of several geo steps; an audio effect inside a series; mirror∘mirror
- **THEN** the single one goes through the old kernel (the goldens stand), the series goes through one fused pass, audio is transparent, the double mirror fuses into identity, the frame size of the series without resampling is integral
- **TEST** t297 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The supersampling boundary is exactly 1 + 1/64
- **WHEN** the length of the image of an output step in the source crosses the minification threshold
- **THEN** the number of taps grows in a quantum of 1/32 px with a ceiling of 4×4, and a single Pan goes into the fused pass only above the threshold
- **TEST** t300 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Point-wise ops inside a run are absorbed by tap ops
- **WHEN** Greenscreen/Noise or ColorGrade/Duotone/Posterize/Vignette stand between geo steps
- **THEN** the plan is one run; the key and sharpness pass through the series, the grain is deterministic, blocky and zoomed together with its stage; a vignette absorbed before the zoom darkens the stage (the zoom cuts the corners); leading/trailing point-wise ops remain barriers, and the bytes of K=1 chains are intact
- **TEST** t305, t330 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: A geo run between K× segments
- **WHEN** a single Mirror stands between two barriers with K=2 ([grid, vhs, mirror, blur, zoom ×2])
- **THEN** the plan is assembled without an assertion: the Legacy route only at equal input and output scales, a change of scale — the fused pass
- **TEST** t308b (`src/libs/kt-fx/tests/geo_fuse.rs`)

### Requirement: Geometry series — one pass of inverse mappings
Consecutive Pan/CamMotion/Mirror/EventGrid/Pan3d/CornerPin/Warp primitives SHALL
be executed in one pass of concatenated inverse maps with supersampling
over the Jacobian under minification. Pan3d SHALL also be computed on CPU (the inverse
f64 homography); without a camera Pan3d is passthrough. CornerPin and Warp primitives
do not require a camera. Neutral parameters of a geo step SHALL be skipped
by the builder — the chain remains identity byte for byte.
Why: the owner's complaint — "a 2×2 grid → a ×2 zoom turned a checkerboard into grey":
each step resampled again; the net unit must return the detail.

#### Scenario: The net unit returns the detail
- **WHEN** a checkerboard passes through [grid 2×2 → zoom ×2] and the perspective variant [grid → pan3d → grid → pan3d ×4]
- **THEN** the contrast returns whole (255) and ≥ 150 for the perspective one; supersampling suppresses the noise variance under minification
- **TEST** t296 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Pan3d on CPU — edges, behind the camera, without a camera
- **WHEN** Pan3d is computed scalar with defaults, with rotation, with a point behind the camera, without a camera in the context
- **THEN** the defaults are identity byte for byte; behind the camera and outside the quad — transparency; the contour is anti-aliased by a fractional alpha; without a camera — passthrough
- **TEST** t298 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: CornerPin — perspective without a camera
- **WHEN** default corners; compression into the right half; a perspective quad; a degenerate quad; a series [grid, corner_pin, zoom]
- **THEN** the default is identity byte for byte without declaring alpha; the marker goes to the assigned corner ±4 px, outside the quad it is transparent; degeneration — a transparent matrix without NaN; the series — one run
- **TEST** t331 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Warp primitives — nonlinear invertible maps
- **WHEN** LensDistort, Twirl, Ripple, Kaleidoscope, Cylinder3d one at a time and in a series [lens, twirl, zoom]
- **THEN** the neutrals are identity byte for byte; the centre of twirl is fixed, the marker travels along an arc; lens converges to the solution rs = r·(1+k1·r²) ±1.5 px; the first half of the kaleidoscope wedge is identical to ≤1 LSB; the centre column of the cylinder is in place; the series is one run with PerPixel supersampling
- **TEST** t333 (`src/libs/kt-fx/tests/geo_fuse.rs`)

### Requirement: K× barriers carry detail; the kernels are resolution-invariant
Barriers (Blur/Glow/Edges/VHS/Noise, Sharpen/Pixelate/ChromaticAberration/
RadialBlur/DropShadow/Halftone) SHALL be executed at K× resolution under "compression
above + zoom below": K = quantum of min(net zoom of the tail, net compression from the start),
ceiling 4, ceiling of the buffer side `MAX_INTER_DIM = 4096` — a project constant,
not a device limit. The kernel MUST be invariant to K (pixel geometry ×K,
the look is the same); at K=1 — the former code verbatim. The stops of the layer's strength zero both
accumulators. The decode scale step of the input SHALL seed the compression accumulator, and the leading
barriers before the first geo run SHALL be forcibly executed in the step.
There are no switches for merge/K either in the UI or in the env.
Why: a barrier in the middle of a series breaks it and earlier "killed" details that the series
could have returned; K is paid only where the detail is really squeezed.

#### Scenario: A barrier does not kill the detail
- **WHEN** a checkerboard passes through [grid 2×2 → vhs(identity) → zoom ×2]
- **THEN** the contrast is ≥ 200 (before K was enabled — 0.0)
- **TEST** t302 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The K rule and the byte anchor
- **WHEN** the tail is empty / does not zoom / compresses; squeeze-cap without a grid above; grid + zoom; neighbouring barriers; Mix between them
- **THEN** K = 1 / 1 / 1 / 1 / 2 / equal / broken; a K=1 chain with a barrier gives a fixed byte hash
- **TEST** t304 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Barrier kernels are invariant to K
- **WHEN** down_K(kernel_K(up_K(F))) is compared with kernel_1(F) at K=2 and K=4
- **THEN** the metric per kernel class is within a threshold ×2 of the measurement: smooth ones — mean/p99, a shadow on a hard alpha — its own thresholds, pixelate — the share of |Δ|>16, halftone — the coverage of 8×8 blocks (pixel-by-pixel comparison is impossible)
- **TEST** t303 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The decode scale step feeds the compression budget
- **WHEN** the frame arrives at S× the logical size
- **THEN** [vhs, zoom2] at S=2 gets K=2 (at S=1 — 1); the leading barrier is executed at S× by the physics of the frame; the former plans at S=1 stay intact
- **TEST** t308 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The meaning of the second-wave barriers
- **WHEN** Sharpen, Pixelate, ChromaticAberration, RadialBlur, DropShadow, Halftone are applied to a test frame
- **THEN** the contrast grows; cells are constant; the centre is the fixed point of the aberration and of the ray; the shadow in the cut-out zone is shifted and tinted, and an input without alpha is passthrough; the pattern is two-level, the density of the dots follows the luma
- **TEST** t332 (`src/libs/kt-fx/tests/geo_fuse.rs`)

### Requirement: The alpha rule applies to each step of the chain
Each video step SHALL relate to alpha in exactly one way: Produces
(creates transparency — Pan3d, Greenscreen, CornerPin), Destroys (guarantees
an opaque output — Edges) or Preserves (stays silent — all geometry, including
EventGrid). The folding is "the last declarer wins" among Produces and
Destroys; Preserves MUST NOT take part in the folding.
Why: EventGrid declared itself Destroys and erased the Produces of the neighbour above —
a title or the output of a greenscreen landed in the composite as an opaque rectangle.

#### Scenario: Folding and pixels
- **WHEN** the chain folds the declarations Produces/Destroys/Preserves; the grid tiles a transparent frame; blur is applied to an alpha frame; opaque frames go by the former path
- **THEN** the folding is correct, transparent pixels stay transparent, the blur has no dark halo, opaque frames are byte-identical to the former ones
- **TEST** t519, t520, t522, t523 (`src/libs/kt-fx/tests/geo_fuse.rs`); the composite — t521 (`src/libs/kt-engine/tests/transitions_video.rs`, capability `video-plan`)

### Requirement: Memoization of the post-FX frame does not break "preview == export"
A video post-FX frame SHALL be memoised in the cache under a key including the hash of the
chain, the geometry of the clip and the resolved camera; a stale value MUST NOT be read after a
change of parameters, a shift of the clip, an edit or a move of the camera. Export MUST NOT
use memoisation. The cold and the warm compose SHALL be byte-for-byte equal to the
export frame.
Why: a chain of four effects is tens of ms per frame, without memoisation a warm loop does not play;
but a cache that sees a static hash and is blind to the per-frame camera gave out stale results.

#### Scenario: Cache, invalidation and export
- **WHEN** a repeated compose of an FX clip; a change of the chain or a shift of the clip; an edit of the camera; the compose of the export
- **THEN** a byte-identical frame from the cache; recalculation without staleness; the camera in the key — a stale value is not read; export skips memoisation
- **TEST** t66, t66c, t66d (`src/libs/kt-engine/tests/postfx_cache.rs`), t67 (`src/libs/kt-engine/tests/golden_export.rs`) — the frame-plan side is in capability `video-plan`

### Requirement: The effect thumbnail is "fit and apply", and no more
The single entry for thumbnails (the kind's icon, a variant cell, a library preset)
SHALL be `fit_into` + execution of the chain with `in_scale = 1` at the REFERENCE size
of the source thumbnail (320×180), followed by shrinking into the cell. A second path in the UI
MUST NOT be introduced. The built-in sample for icons SHALL be frozen
by a golden and give material to each class of effects; the sample SHALL have
a variant with transparent margins. The icon of a kind with an identity default SHALL
be the first variant of the registry. The grain of noise SHALL be pinned to the tick.
Why: the picture in the picker must coincide with what results after a click;
pixel parameters (cell size, radii) at 96×54 and at 1080p give different
strength — the calculation must be done at the reference size.

#### Scenario: The sample is frozen and deterministic
- **WHEN** the sample is generated twice in three sizes/modes
- **THEN** the hashes equal the reference, the sizes and the alpha flag are correct
- **TEST** t544 (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: The sample feeds every class of effects
- **WHEN** a mirror on any axis, greenscreen, colour, blur/sharpen/edges/pixelate/raster are applied to the sample
- **THEN** each result differs from the input, and greenscreen produces pixels with zero alpha
- **TEST** t544b (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: Identity remains identity, the variant does not
- **WHEN** the thumbnail is computed for the identity default of a kind and for its variant with zoom
- **THEN** the default equals the fitted sample, the variant differs
- **TEST** t544c (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: A shadow requires an alpha sample
- **WHEN** DropShadow is computed on an opaque sample and on an alpha sample
- **THEN** on the opaque one — passthrough, on the alpha sample — not
- **TEST** t544d (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: The thumbnail equals the manual pair "fit + chain"
- **WHEN** the thumbnail is compared with `fit_into` and with direct execution of the chain at the reference size
- **THEN** the bytes match
- **TEST** t544e (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: The grain is pinned to the tick
- **WHEN** a thumbnail with noise/VHS is computed again at the same tick and at a different one
- **THEN** the same tick — the same bytes; a different tick — different ones
- **TEST** t544f (`src/libs/kt-fx/tests/fx_thumb.rs`)

### Requirement: Built-in variants — a named configured state
Each video kind in the picker registry SHALL have 2–3 built-in variants —
instances of the same kind with parameters set BY NAME through
`set_param`; a variant MUST differ from the template, stay within the bounds of the spec,
and have a name in the five translation catalogs. The order is from weak to strong, the first
cell MUST NOT be identity. The check of bounds is strict: if you hit the f32
ceiling — move the value back, do not weaken the test.
Why: the thumbnail strip needs WHAT to show; library presets do not belong to the kind.
A typo in a parameter name silently makes a variant a copy of the template.

#### Scenario: The variants are distinguishable and within the spec
- **WHEN** the variants of all video kinds of the registry are enumerated
- **THEN** each differs from the template, all numeric values are within the specs after `set_param`, the names are translated into all languages
- **TEST** t543 (`src/apps/kt-app/src/fx_registry.rs`)

### Requirement: The thumbnail service is a service, not a task
Thumbnails SHALL be computed only by a dedicated service: its own thread, a condvar LIFO queue
with deduplication, cancellation by generation, a RAM LRU of 256 cells. The base frame SHALL
be resolved by the caller and passed as a ready picture: the service has no decoder,
sidecar DB and persistence. The cell key is (base, recipe); the recipe is the hash of the `Debug`
of the chain plus the moment, the grain and the camera; an edit of the generator's parameters MUST
invalidate the cells of this media. The 800 ms mis-click guard SHALL guard
DECODE ORDERS, not the rendering from ready frames. The egui texture cache lives in the service.
Why: scrolling/zooming and opening the picker do not generate decode requests (the cache invariant),
and the expensive half of the preview — decoding — already lies in the thumbs.

#### Scenario: Queue, LRU, recipe, delivery, guard
- **WHEN** requests are placed, the generation grows, the LRU overflows, the chain/moment/camera changes, the media is edited, a missing frame is requested before the guard expires
- **THEN** the latest request goes first, duplicates collapse, the old generation is cancelled, the oldest is evicted, the recipe distinguishes only what changes the picture, the media cells are reset, decoding waits for the guard, and rendering from ready frames does not
- **TEST** t546 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: Render path — request first, then texture
- **WHEN** the picker context asks for a cell's texture
- **THEN** first a request is placed and `None` is returned; after the worker finishes, the texture is returned
- **TEST** t546g (`src/apps/kt-app/src/fx_thumbs.rs`)

### Requirement: Picker gallery — WYSIWYG, and the click reaches the model
The effect row SHALL show the category icon, the name and a strip of cells
(variants and the suitable single-effect presets); the audio picker stays
text-only. A cell SHALL be counted on the event's chain PLUS the candidate at the tail,
at the cursor tick (frozen only during playback), with curves and camera following the
same rules as the engine; base frame: exact thumb → poster → sample, there are no
empty cells. The popup MUST close only by a click outside or by an explicit
choice; a cell below the scroll fold MUST NOT be drawn or requested.
Categories are grouped once at draw time; the caption is truncated to one line;
the accessible name of a cell is compound ("Blur Heavy").
Why: the egui menu default is "close on any click" — the gallery would be stillborn;
an invisible cell published a rectangle to the timeline, and the
live scenario clicked past the popup.

#### Scenario: A click on a cell reaches the model
- **WHEN** a click on a variant cell inside the popup
- **THEN** an effect with the parameters of exactly this variant appears in the event's chain; input inside the popup does not close it
- **TEST** t547 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Layout is stable and truncated
- **WHEN** the gallery is laid out with long names, repeated categories, and in the audio picker
- **THEN** the caption is truncated and no wider than the cell, the category heading appears once, the width does not jump with the query, and audio cells have none
- **TEST** t548 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Moment and camera of the strip — by engine rules
- **WHEN** the strip computes the event tick for a reversed clip and resolves the camera for the Pan 3D variant
- **THEN** the map "event tick → source frame" is one and the same as for the filmstrip and waveform and knows about reverse; the camera rule is one: pinned → active → default
- **TEST** t549 (`src/apps/kt-app/src/thumbs.rs`), t545 (`src/libs/kt-engine/tests/cam_resolve.rs`, capability `video-plan`)

### Requirement: Strength axis — a mix of curated neighbours; a click gives a variant
Horizontal drag over a cell SHALL lead a blend of the two neighboring points of the row
[template, variant₀, variant₁, …] and put into the model exactly what is shown; a click without
movement MUST give exactly the named variant. The axis is quantized (8 steps per interval), the edges stop
the drag, and the count starts from the center of the cell by the absolute pointer position.
Blending two states of one kind: numbers linearly, non-numeric values from the nearest end, the ends byte-for-byte; different kinds are not blended; a blend
stays within the spec across the whole registry. "Main parameter" for a kind is NOT declared.
Why: VHS and Color Grade have four equal parameters, Mirror has no numeric ones, Corner Pin has eight coordinates — a main parameter cannot be declared;
the variants already define a useful range.

#### Scenario: Drag puts a blend, a click puts a variant
- **WHEN** a drag from "Heavy" by half a cell to the left; a click without movement
- **THEN** the model holds a radius between "Soft" and "Heavy"; the click gives exactly "Heavy"
- **TEST** t551 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Row arithmetic
- **WHEN** a shift by a fractional number of cells, past the left and right edges
- **THEN** the position is quantized to 8 steps and clamped to the edges
- **TEST** t551b (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Blending states of one kind
- **WHEN** two instances of one kind are blended at t = 0, 0.5, 1, and for each kind in the registry
- **THEN** the ends are byte-for-byte, non-numeric values come from the nearest end, different kinds are not blended, the result is within the spec
- **TEST** t550 (`src/libs/kt-model/tests/fx_lerp.rs`)

### Requirement: The "Geometry" node — leading, mandatory, movable
Each video event SHALL have exactly one LEADING "Geometry" node in the chain (the conform rule
"follow the project" or its own, crop in fractions of the source, position in fractions of the canvas,
zoom, rotation, anchor); only the leading node carries the rule. Deleting the leading node SHALL
be rejected with a reason; reordering along the chain SHALL be an ordinary operation; a "Geometry" node without
a rule SHALL be addable from the catalog any number of times. Effects BEFORE the leading node SHALL
run in source space (a buffer of decode size), AFTER it — on the canvas; Pan3d and
CamMotion remain effects and by default stand above the conformed frame. The kind `Pan` SHALL
be absent from the registry; a leading node with defaults SHALL be an identity and SHALL share the
identity hash with the earlier identity kinds. The order inside the node — crop, (place for the mask), placement — is fixed, so that a mask does not rewrite the node.
Why: in Vegas, Pan/Crop is the first node of the chain and draggable ("effects applied before
or after any event panning and cropping"), and the owner asked for "not worse, but cooler"; the dead
combo of three Pan modes is exactly the extra widget of rule 9.

#### Scenario: The leading node cannot be deleted, is moved, is added again
- **WHEN** the user deletes the leading node, moves it after Blur, adds a second "Geometry" node, and `FxInstance::all()` is checked against the golden table
- **THEN** the deletion is refused with a reason; the move and the second node succeed; the second node has no rule; `Pan` is absent from the registry; the node with defaults gives the identity hash
- **TEST** t716 (`src/libs/kt-model/tests/fx_chain.rs`), t328 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: Before the node — source space, after — canvas
- **WHEN** on a vertical source the chain is Blur → Geometry and Geometry → Blur; the vertical source is rotated 90° into 16:9
- **THEN** the frames differ (blurring the field versus blurring the material), both backends are within tolerance; the rotated source fills the canvas without margins
- **TEST** t717 (`src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`)
