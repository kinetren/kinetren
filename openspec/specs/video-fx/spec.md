# Video effects

## Purpose

A registry of 27 kinds of video effects (point-wise, barriers, geometry, 3D), a single
chain planner with a CPU reference and a GPU mirror, and the picker gallery with
thumbnails, variants and the strength axis — the picture in a cell equals the result of a click.

## Requirements

### Requirement: The video effect registry is closed by a golden cycle
Every video kind of the CLOSED built-in set — the core-owned kinds (`Geometry`, `CamMotion` as a
legacy reading) plus the built-in video plugins listed by the assembler (capability `plugins`) —
SHALL have a row in the golden table of default templates, and the table SHALL cover that set in
both directions: a new built-in kind does not build (or does not pass the completeness lock)
until its row is added. Every kind MUST declare its class to the planner (point-wise / barrier /
geo): a core kind by its arm, a plugin by its manifest — an undeclared kind is refused at
registration, and a core kind with a forgotten arm is caught by a panic when run through the
barrier path. The default of any new parameter MUST give the former behaviour byte for byte: the
goldens are not regenerated when parameters are added, and they are not regenerated when a kind
moves from the enum into a plugin crate. The defaults of "identity" kinds (Geometry, Pan 3D,
Color Grade, Corner Pin, Drop Shadow on an opaque frame) legitimately share one identity hash;
the kind `Pan` was removed from the registry, and it has no row in the table. A video plugin
registered outside the built-in set MUST NOT have a row and MUST NOT be required to have one.
Why: "a new variant compiles and silently does nothing" — a forgotten arm of the
classification became a barrier and broke geo series; the completeness lock makes this
a build error, not a discovery of the user. With plugins the class moves into the manifest, and
the lock moves to registration; the references stay a property of the product (F-EXT-003).

#### Scenario: A new kind will not build without a row
- **WHEN** a video kind is added to the built-in set without a row in the golden table or with a forgotten class
- **THEN** the golden loop fails on the completeness lock (or the registry refuses the manifest, or the debug panic of the barrier path fires for a core kind), and the hashes of all former defaults stand
- **TEST** t328 (`src/libs/kt-fx/tests/video_fx.rs`), t972 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: A moved kind keeps its golden hash
- **WHEN** a video kind has moved from the enum into its family crate (the pilot Posterize first, then each family stage)
- **THEN** its row in the golden table is unchanged and green, the table is still complete over the assembler's closed list, and a plugin from the test door is not in the table
- **TEST** t328 (`src/libs/kt-fx/tests/video_fx.rs`), t976 (`src/apps/kt-app/src/reach_tests.rs`)

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
The CPU kernels (`video.rs`, `geom.rs`, `plan.rs`, and the CPU kernel of each video plugin) SHALL
be the reference of determinism and the full fallback, including 3D. The GPU mirrors SHALL be
checked against the CPU only through the shared tolerance helper with tolerance classes (EXACT /
BILINEAR / THRESHOLD / GEOM); an ad-hoc epsilon MUST NOT appear. An edit of the formula of a
kernel, of the planner or of a geo primitive MUST be accompanied by an edit of the WGSL mirror
(including `geom_fused.wgsl`) in the same commit; for a plugin the CPU kernel and its WGSL piece
SHALL live in the same family crate, so the pair is one crate and one commit. The fused geometry
pass and the per-kind passes SHALL be assembled from the registry's WGSL pieces when the GPU
executor is created, and the assembled text SHALL be a pure function of the registry. The SIMD
kernel MUST be byte-for-byte equal to the scalar one; the scalar is the definition of the bytes
and is not refactored together with the kernel.
Why: two executors of one formula silently diverge; the conformance loop over
the registry and the paired edit are the only way to keep them together. Keeping both halves in
one plugin crate turns the "same commit" rule into a property of the crate layout.

#### Scenario: Conformance per effect and per chain
- **WHEN** each kind, geo series, pan3d chains, tap ops and K× barriers are computed on GPU and CPU
- **THEN** the divergence fits within the tolerance class of the kind; geo series and Pan3d — within GEOM
- **TEST** t181 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The conformance loop over the whole registry
- **WHEN** each video kind of the built-in set is run alone and "sandwiched" [grid, effect, zoom ×2], plus configured instances of identity defaults and warp-hard cases
- **THEN** the tolerance class is set by a table with a completeness lock over the built-in set; a failure of warp-hard — investigation and introduction of WARP from a measurement, not tuning GEOM
- **TEST** t329 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The assembled shader joins the one path
- **WHEN** the GPU executor is created twice from the same registry, and the pilot point-wise plugin runs alone, inside a fused series and as a barrier position
- **THEN** the two assembled shader texts are identical, the fused series is one pass, and every frame is within the point-wise tolerance of the CPU reference
- **TEST** t974 (`src/libs/kt-gpu/tests/parity.rs`)

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
Barriers — every registered video kind of the barrier class, built-in or plugin, with no per-kind
list (today blur, glow, edges, vhs, noise, greenscreen, sharpen, pixelate, chromatic_aberration,
radial_blur, drop_shadow, halftone) — SHALL be executed at K× resolution under "compression
above + zoom below": K = quantum of min(net zoom of the tail, net compression from the start),
ceiling 4, ceiling of the buffer side `MAX_INTER_DIM = 4096` — a project constant,
not a device limit. The kernel MUST be invariant to K (pixel geometry ×K,
the look is the same); at K=1 — the former code verbatim. The stops of the layer's strength zero both
accumulators. The decode scale step of the input SHALL seed the compression accumulator, and the leading
barriers before the first geo run SHALL be forcibly executed in the step.
There are no switches for merge/K either in the UI or in the env.
K SHALL be decided by the host planner and handed to the kernel; a barrier kernel declares no K
rule of its own.
Why: a barrier in the middle of a series breaks it and earlier "killed" details that the series
could have returned; K is paid only where the detail is really squeezed. The rule is the host's so
that a barrier plugin unknown to the host is held to it by construction (plugin-api-barriers).

#### Scenario: A barrier does not kill the detail
- **WHEN** a checkerboard passes through [grid 2×2 → vhs(identity) → zoom ×2]
- **THEN** the contrast is ≥ 200 (before K was enabled — 0.0)
- **TEST** t302 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The K rule and the byte anchor
- **WHEN** the tail is empty / does not zoom / compresses; squeeze-cap without a grid above; grid + zoom; neighbouring barriers; Mix between them
- **THEN** K = 1 / 1 / 1 / 1 / 2 / equal / broken; a K=1 chain with a barrier gives a fixed byte hash
- **TEST** t304 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: Barrier kernels are invariant to K
- **WHEN** down_K(kernel_K(up_K(F))) is compared with kernel_1(F) at K=2 and K=4 for every barrier kind the registry holds
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

#### Scenario: A barrier plugin unknown to the host is held to the K rule
- **WHEN** a test barrier plugin, registered beside the built-ins and named nowhere in the host, stands between a 2×2 grid and a ×2 zoom, and then alone after a decode scale step S=2
- **THEN** the planner gives it K=2 in both cases, the same K a built-in barrier gets at that place, and its kernel receives that K; the series around it is broken exactly as for a built-in
- **TEST** t1055 (`src/libs/kt-fx/tests/barrier_plugin.rs`)

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

### Requirement: The effect catalogue is measured against the market and grows in ranked waves
The video effect catalogue SHALL have one reconnaissance note (F-FX-007,
`docs/features/fx/catalogue/market-catalogue-and-waves.md`) that lists EVERY built-in kind by its
id exactly as the registry has it (video effects, audio effects, the layer marker, transitions,
generators), groups what other editors offer by what people use, and gives each group a verdict
from a closed vocabulary: `have`, `wave Wn`, `deferred (<dependency>)`, `rejected (<reason>)`. A
row whose market side could not be checked live SHALL carry `[NOT VERIFIED]`. New video kinds
SHALL arrive only through a wave of that list: each wave is its own change, built as plugin
family crates under `src/plugins/` (capability `plugins`), and it names the note ids it depends
on. A wave that brings back a kind or a mode listed under "Rejected and why" of the video-fx,
transitions or audio-fx design SHALL carry the line `Revision: <decision> — <reason>`. Every FX
item of the owner's claims files (ktprompts) SHALL have a row with a verdict.
Why: the owner — "we are slipping badly on effects and transitions, far behind CapCut and the
like" (2026-10-27). A catalogue that is not compared with the code drifts the day the next kind is
added; a wish list without a rank is how the "67 niche plugins" become 67 half-done kernels; and
two of the obvious names (a directional blur, sepia/invert kinds) were rejected earlier with
reasons that must be answered, not forgotten.

#### Scenario: The note lists exactly the built-in kinds
- **WHEN** the guard reads the five "Our catalogue" lists of F-FX-007 and the registries
  (`FxInstance::all()` split by class, the layer marker, `TransitionKind::ALL`, the generator
  tags — after `plugin-api-stage1`, the assembler's closed list plus the core-owned kinds); then a
  temporary copy of the note with one id removed and one id invented is checked
- **THEN** each list equals its registry as a set and the counts in the list headings (18, 1, 27,
  33, 15 today) equal the registry sizes; both broken copies are red and the message names the
  missing and the extra id
- **TEST** t978 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: The wave list names only existing notes and capabilities
- **WHEN** the guard reads the Verdict of F-FX-007 and the waves of the change design, and
  collects every `F-XXX-NNN` id and every `openspec/specs/<cap>/` path they name; then a copy with
  an invented id `F-FX-099` is checked
- **THEN** every id is the `id:` of an existing note under `docs/features/` and every capability
  directory exists with a `spec.md`; the copy is red and names `F-FX-099`
- **TEST** t979 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: Every catalogue row has a verdict from the closed vocabulary
- **WHEN** the guard reads the "What people use, against what we have" table of F-FX-007
- **THEN** every row's last cell starts with `have`, `wave W<n>`, `deferred (` or `rejected (`;
  every `wave W<n>` names a wave that exists in the Verdict list; a `rejected (` or `deferred (`
  cell carries a non-empty reason or dependency; a market cell either cites a numbered source of
  the note's Sources list that was opened live (`(src N)`, the source carrying no `[NOT VERIFIED]`)
  or carries `[NOT VERIFIED]` itself; copies with an unknown verdict, an unknown wave, an empty
  reason, an unmarked unchecked row and a cite of an unopened source are each red with the group
- **TEST** t980 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: Reviving a rejected idea is an explicit revision
- **WHEN** the guard reads the "Rejected and why" sections of `openspec/specs/{video-fx,transitions,audio-fx}/design.md`,
  takes the closed list of rejected names kept in the test (directional blur, sepia/invert as
  kinds, a node graph, a transition progress curve, stylised audio transitions, a gallery of live
  transition previews), and scans the waves of F-FX-007 and of the change design for them
- **THEN** every wave that names one of them carries a line `Revision: … — …` in the change
  design; a copy where W2's directional blur loses its revision line is red
- **TEST** t981 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: The owner's FX items all have a row
- **WHEN** the guard takes the closed list of FX items of `ktprompts_v26.10` recorded in the test
  (fisheye lens corners; dirt/drops/flares on the lens; letters as physical objects; inertia of
  letters under rate; the "67 niche plugins" reference; jag edit by time remap; parallax; text
  blending and shapes holding sources; homothety; bump mapping; buffer + blend levels; audio-driven
  zoom; blur on one word of a ticker; fisheye switched on by speed changes)
- **THEN** each item is found in the note's table or its "owner's items" list with a verdict; an
  item removed from the note makes the guard red with the item's name
- **TEST** t983 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

### Requirement: The VHS tape simulation writes the signal onto a modelled tape and reads it back
The kind `vhs_tape` SHALL compute its picture by the physical chain of a VHS recorder and player
(`docs/features/fx/simulate/vhs-tape-simulation.md`), not by image operations:
- the frame is resampled onto the standard's signal grid at 4 fsc (PAL 1135×576, NTSC 910×480);
- the luma is pre-emphasised (τ 1.3 µs, ×5), clipped and frequency-modulated between the sync tip
  and white carriers (PAL 3.8–4.8 MHz, NTSC 3.4–4.4 MHz) by a phase accumulator;
- the chroma is band-limited and recorded as colour-under (626.953 / 629.371 kHz) with the per-track
  rotation (PAL 0 / −90° per line, NTSC −90 / +90°) under the FM, through tape saturation;
- each row is read by the head of its field through the channel of that head: spacing loss, gap loss
  and, for the strip of the neighbour track under the head, the azimuth loss of the ±6° gaps, with
  gains given by the head-to-track geometry (track pitch per speed, head widths of a two- or four-head
  drum, tracking offset, scan-angle mismatch, still), plus tape and amplifier noise and dropouts;
- the output switches heads at the switching point before the field's end;
- playback FM-demodulates, compensates dropouts, de-emphasises, demodulates the colour-under with the
  track's rotation undone, applies ACC and the 1 H (NTSC) / 2 H (PAL) comb, and reads the rows with
  the time-base error of the tape unless a TBC is on.
Its parameters SHALL be physical quantities in their units (µm of spacing, gap, tracking and skew;
µs of dropout length, switching skew and jitter; dB of head output and C/N; per head and per drum;
the standard, speed, deck count, head count, transport, TBC, DOC and comb). With one deck the same
drum records and plays; with two, the record drum's head defects are written into the tape and the
play drum adds its own. The output is opaque. The imitation `vhs` stays unchanged.
Why: kkt-26.11.01 V1, the owner: "really take a VHS tape, really write a magnetic signal onto it,
then really read it back"; answer 4: "you … completely ignore the complexity of the effects'
physical mechanics". The root rule: an imitation is not a simulation.

#### Scenario: Without impairments the FM round trip gives back the luma
- **WHEN** a luma ramp and a bar pattern pass through the chain with ideal heads (no spacing, a
  minimal gap, the head exactly on its track, no noise, no dropouts, no chroma, no saturation)
- **THEN** the demodulated, de-emphasised luma equals the record-side band-limited luma within 1.5 %
  of the white level away from the clipped edges, and the FM carrier measured on the written signal
  is the standard's sync-tip and white frequency within 1 %
- **TEST** t1180 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Colour-under halves nothing of the luma and cuts the chroma
- **WHEN** a horizontal sinusoid sweep is put into the luma, then into U, and measured at the output
- **THEN** the luma passes 1.0 MHz within 3 dB of its 0.2 MHz level; the chroma at 1.0 MHz is at
  least 12 dB below its 0.2 MHz level (the colour-under bandwidth ~0.5 MHz)
- **TEST** t1181 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: A clogged head kills every other field
- **WHEN** the playback head A's spacing is 3 µm (head B clean)
- **THEN** the even rows (field A) are noise — their error against the clean run is at least 8×
  that of the odd rows — while the odd rows stay within the clean run's error
- **TEST** t1182 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The tracking knob moves the noise bar
- **WHEN** two decks with a scan-angle mismatch (skew) play with the tracking offset at two values
- **THEN** the noise per row has a bar where the head leaves its track, and the bar's position moves
  in the direction and by the amount the geometry predicts (Δu = −Δtracking / skew, within 0.06 of a
  field); at a centred tracking with no skew there is no bar
- **TEST** t1183 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Azimuth rejects the neighbour's FM and lets its chroma leak, which the comb cancels
- **WHEN** the neighbour-track kernel of a row with a strip w of the neighbour under the head is
  measured at the FM and colour-under frequencies, and a picture whose two fields differ is played
  with the head half on the neighbour, with the comb on and off
- **THEN** the kernel's transfer follows sinc(π·w·tan 12°·f/v) within 1.5 dB at both frequencies
  (≥ 20 dB more loss at 4.3 MHz than at 0.63 MHz); the neighbour's luma leaks less than its chroma;
  the comb reduces the chroma leak by at least 10 dB
- **TEST** t1184 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Time is the only state
- **WHEN** the same frame is rendered twice at one timeline position, at two positions, and a sequence
  of positions in reverse order
- **THEN** one position gives identical bytes, two positions give different noise and dropouts, the
  reverse order gives the same bytes per position; the kernel calls no platform transcendental
- **TEST** t1185 (`src/plugins/fx-video-tape/tests/physics.rs`), t1066

#### Scenario: The GPU graph is within its tolerance of the CPU
- **WHEN** the kind runs on the GPU executor at its defaults and at each preset, PAL and NTSC, at K=1
  and K=2, on a frame with alpha
- **THEN** the result is within `FrameTol::SIGNAL` of the CPU, and the graph is deterministic on the
  device
- **TEST** t1186 (`src/libs/kt-gpu/tests/vhs_tape.rs`)

#### Scenario: Still on two heads shows a bar, on four heads it does not
- **WHEN** the transport is still on a two-head and on a four-head drum
- **THEN** the two-head picture has a noise bar inside the field (the scan crosses one pitch); the
  four-head picture has none and its two fields carry the same lines
- **TEST** t1188 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: LP is noisier, not softer
- **WHEN** the same picture is played at SP and at LP on a two-head drum
- **THEN** the noise at LP is at least 2 dB higher, and the luma response at 1 MHz is the same within
  1 dB (the writing speed, hence the wavelength, is unchanged)
- **TEST** t1189 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: A defect of the recorder survives a clean player; a chipped head flickers
- **WHEN** two decks play, the RECORD drum's head A clogged and the play drum clean; and one deck with
  head B chipped (output −6 dB, a 0.6 µm gap)
- **THEN** field A is noise in the first case although the playing heads are clean; in the second the
  odd rows are noisier than the even rows by at least 6 dB
- **TEST** t1190 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The TBC removes the line jitter and the switching skew, not the switching noise
- **WHEN** a picture with vertical edges is played with jitter and switching skew, with and without a
  TBC, and the switching point is moved
- **THEN** without the TBC the edge position varies between rows and the rows after the switching
  point are displaced by the skew; with the TBC both are gone; the rows affected follow the
  switching point
- **TEST** t1191 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The dropout compensator hides dropouts with the line before
- **WHEN** a worn tape with many dropouts is played with the compensator on and off
- **THEN** the error of the rows with dropouts is at least 2× lower with the compensator on
- **TEST** t1192 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The effect window shows the deck face
- **WHEN** the effects window of an event with the VHS tape simulation is opened, and the head
  selector picks head B and sets it to "chipped"
- **THEN** above the parameters a deck face shows the drum with its heads, the speed switch, the
  tracking knob and the deck count; the chosen head is drawn in the chipped colour and the passport
  values of that head changed to the chipped preset's
- **PATH** select an event → Effects (+ effect) → pick "VHS tape (simulation)" → the effects window
  shows a deck face (a drum with two heads A and B, SP/LP/EP, a tracking knob) over the parameter
  grid → click head B on the drum → the head selector names B → pick "Chipped" → head B turns the
  chipped colour, its output and gap fields change
- **TEST** t1193 (`src/apps/kt-app/src/vhs_tape_tests.rs`)

### Requirement: The datamosh is a motion-compensated codec run inside the effect and broken by its rows
The kind `datamosh` SHALL be a SIMULATION of a broken inter-frame stream, never an image
operation: it SHALL contain its own block codec — Y'CbCr 4:2:0 planes at its encoded height,
block-matching motion estimation within a search range, a closed prediction loop over a
reference buffer, a DCT residual quantised by QP and a GOP with I-frames at the stream start, at
the cut and every `gop` frames (0 — only at the stream start, the cut and the resets). Its breakages SHALL be rows of
its passport, and every quantity that may change from frame to frame SHALL take curves: the gates
`mosh` (I-frames dropped while on), `reset` (a key rising to on forces an intact I-frame), `bloom`
(the run's first P-frame repeats), `freeze` (the reference stops updating), the quantiser, the vector
scale and rotation, the residual kept and the macroblock-loss probability. Only the structural rows
take none: the GOP, the block, the search range, the encoded height, the frames from the event before,
the second input (the event on the track above or below), the vector swap and the seed. No range is
capped below what the quantity physically allows: GOP to 600, search to 64 px, 300 frames before the
cut, 144–1080 lines, vector scale ±16; the quantiser keeps the codec's own 0–51.

Why: the owner asked for "a real motion-compression path that we can break carefully with
parameters", not the glitch imitations we have (F-FX-011; root rule imitation ≠ simulation); on the
first build: "no curves … no way to reset the effect, at least by keys on the curves … on the beat …
the GOP is limited to 60 for some reason, and probably the other parameters too".

#### Scenario: An intact stream reproduces its input within the quantiser
- **WHEN** a moving picture goes through the codec with no breakage at QP 10 and at QP 30
- **THEN** every decoded plane differs from the source plane by no more than the quantiser's bound, the mean error grows with QP, and the error does not grow along the GOP (closed loop)
- **TEST** t1235 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A dropped I-frame at the cut moves the previous shot with the new shot's motion
- **WHEN** the stream starts in the event before (shot A, a still texture) and the `mosh` gate is on at the cut into shot B (another texture moving right), so the cut's I-frame is dropped
- **THEN** after the cut the output keeps A's texture, displaced by B's motion, and does not show B; with the I-frame intact the output is B within the quantiser
- **TEST** t1236 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: Bloom repeats one vector field
- **WHEN** the `bloom` gate is on over frames b … b+N
- **THEN** each frame b+1 … b+N is the previous output predicted by packet b's vectors (plus its residual), so the moving region keeps travelling by the same displacement, and after b+N the stream continues with its own packets
- **TEST** t1237 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A frame is the same bytes however it is reached
- **WHEN** frame N inside a mosh — constant gates, a bloom run, a freeze run, a beat of mosh on / reset key / mosh off drawn by curves — is rendered after rendering 0 … N−1 in order, after a seek with an empty memo, with a memo of 1 byte, and twice on two threads
- **THEN** the bytes are identical in every case
- **TEST** t1238 (`src/plugins/fx-video-codec/tests/codec.rs`), t1239 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The other breakages change the stream they name
- **WHEN** the `freeze` gate is on over a run, the vectors are taken from a second input, the vectors are scaled by 0 or swapped, and macroblocks are lost with probability 0 and 1
- **THEN** a frozen run predicts every frame from its first picture and leaves its trace after it; transplanted vectors move the own picture by the second input's motion; scale 0 holds the picture; probability 0 equals the clean stream and probability 1 differs from it, deterministically per seed
- **TEST** t1243 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The person resets the mosh on the beat with curve keys
- **WHEN** the `mosh` gate is on and a `reset` key rises to on at frame 10; and, in another stream, `mosh` turns off at frame 5 with a GOP of 12
- **THEN** frame 9 is the previous shot dragged, frame 10 is the new shot's intact keyframe and frame 14 is the new shot; in the other stream the cut's keyframe was dropped (frame 9 moshed) and the keyframe at 12 is intact; the curve values are part of the keys
- **TEST** t1245 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A region of interest confines the mosh
- **WHEN** the host gives a region covering the left half of the frame (a stand-in for a mask or an object detector)
- **THEN** inside it the frame is the broken stream's, outside it the intact stream's, byte for byte; without a region the whole frame moshes; the region's identity is in the output key
- **TEST** t1244 (`src/plugins/fx-video-codec/tests/codec.rs`)

### Requirement: The engine runs a stream stage in the plan's common code with random access
A layer whose chain holds a `Stream` slot SHALL take as its pre-FX frame the stage's output,
computed in the plan's common code before either executor runs, fitted to the layer's decode size
and opaque; the slot SHALL be removed from the chain the executors see. The order SHALL ask the
stream's frames at the codec's size — the frame at N, and the history the memo does not cover —
and no frame at the ordinary decode quality. The layer's content key SHALL carry the decoded
frame's key. Export SHALL pre-roll the codec before a frame whose history the memo lacks, in
batches bounded by its cache budget. A stream slot on a reel instance or a track stage SHALL be
skipped.

Why: invariants 4, 7 and 11 — the order equals the read, one composition code, the same bytes
whatever the start (`datamosh-codec-path` design D3–D6).

#### Scenario: A reset key on the event ends the mosh on the beat
- **WHEN** the event carries a `reset` curve rising to on at a frame, with the `mosh` gate on
- **THEN** the engine reads the slot's rows per frame with their curves: before the key the frame is far from the intact codec's, on and after it close to it; a seek to a frame after the key equals playing up to it
- **TEST** t1246 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: Seeking to a frame equals playing up to it
- **WHEN** a project with a cut-mosh is composed at frame N after composing 0 … N−1, and by a fresh renderer with a fresh memo directly at N
- **THEN** the frames are byte-identical, and differ from the same project without the slot
- **TEST** t1241 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: Export in chunks equals export in one go
- **WHEN** a range containing a mosh is exported in one go, and again as three range exports — each with a fresh renderer, memo and cache, one starting inside the mosh (harder than a chunk boundary, which keeps the memo)
- **THEN** the concatenated pieces are byte-identical to the whole
- **TEST** t1242 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: The order asks what the stage reads
- **WHEN** `layers_at` is asked at a frame inside a mosh with an empty memo, and again after that frame was composed
- **THEN** the first answer holds the history at the codec size (the event before included) and no frame at the layer's ordinary quality; the second holds only the next frame's own source
- **TEST** t1248 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A person moshes a cut in the editor
- **PATH** add two clips back to back on one track → select the second → FX → pick Datamosh "Cut mosh" → play across the cut → the first clip's picture keeps moving with the second clip's motion after the cut
- **WHEN** the live scenario runs on our fixtures
- **THEN** the frames after the cut differ from the same frames without the effect, and the screenshots of each step are saved
- **TEST** live `datamosh.py` (`src/scripts/live/datamosh.py`, via `src/scripts/live-check.sh`)

### Requirement: The mask rasterizer — shapes, antialiasing, feather with falloff, expansion, modes
One kt-fx function SHALL turn an ordered mask list, an affine "reference-normalized → raster px" and a
raster size into an 8-bit coverage: a rectangle (with corner roundness), an ellipse and a closed cubic
Bézier path are flattened deterministically; the inside is the nonzero winding at pixel centres; a
signed distance to the contour is computed in a band around it; one mask's coverage is a one-pixel
ramp without feather and `falloff(clamp(0.5 + d/feather))` with it (centred on the contour; falloff
linear or smoothstep), where `d` includes the expansion; then invert and opacity. Masks SHALL combine
top to bottom by the soft Boolean forms: add `a+m−am`, subtract `a(1−m)`, intersect `am`, difference
`a+m−2am`, starting from empty — or from the full frame when the first mask subtracts or intersects.
Feather and expansion are in px of the reference frame and scale with the raster, so a mask covers the
same region at every preview quality. Only exact IEEE operations and the frozen math are used, so the
coverage is the same on every platform; a bounded memo keyed by the static masks, the affine and the
size MUST be invisible to the bytes.
Why: F-MSK-001 — feather and expansion are the classic controls; resolution invariance is why the
coordinates are normalized; determinism is invariant 11.

#### Scenario: Shapes cover their area with antialiased edges
- **WHEN** a rectangle, a rotated rectangle, a rounded rectangle, an ellipse and a Bézier path approximating a circle are rasterized without feather
- **THEN** the coverage sum equals the analytic area within 0.5 %; edge pixels take intermediate values, pixels a pixel away from the contour are 0 or 255; the Bézier circle matches the ellipse within one LSB on 99 % of pixels
- **TEST** t1266 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: Feather, falloff, expansion and resolution
- **WHEN** a disc is rasterized with feather 0/20/80 px, both falloffs, expansion −10/+10 px, and the same mask at 1/4 and at 1/1 of the raster
- **THEN** the profile across the edge is monotonic, the 50 % level stays on the contour (moved by the expansion exactly), a wider feather is wider, smooth is steeper in the middle than linear; the 1/4 raster equals the 1/1 raster box-downsampled within 3 LSB
- **TEST** t1267 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: Modes, invert and opacity
- **WHEN** two overlapping hard rectangles are combined in each mode, in both orders, with the first one inverted, and at 50 % opacity
- **THEN** the four regions (A only, B only, both, none) take the truth-table values of OR, AND NOT, AND, XOR; a first subtract/intersect starts from the full frame; invert swaps 0 and 255; opacity scales the mask's own contribution
- **TEST** t1268 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: The coverage is deterministic and the memo is invisible
- **WHEN** a fixed scene of masks is rasterized twice, with and without the memo
- **THEN** the bytes equal a hash frozen in the test, and are equal with and without the memo
- **TEST** t1269 (`src/libs/kt-fx/tests/masks.rs`)

### Requirement: The camera effect simulates a sensor and an ISP, stage by stage
The built-in barrier plugin `camera_isp` (family `src/plugins/fx-video-camera`) SHALL take the
frame as a picture another camera DEVELOPED, SHALL unprocess it into a scene in linear light (the
source's transfer curve, saturation matrix and white balance undone), and SHALL produce the picture
a simulated camera records of that scene: the lens (defocus, distortion, lateral CA, cos⁴
vignetting, colour shading, the 4-spot OLPF); a colour filter array of the layouts RGGB, BGGR,
GRBG, GBRG, Quad Bayer, X-Trans, RGBW and RGB-IR, with spectral crosstalk, Gr/Gb imbalance and an
IR-cut filter whose absence lets the estimated near-infrared into every pixel through the dyes' NIR
transmission; a sensor in electrons (quantum efficiency per channel, full well, Poisson shot noise,
dark current with DSNU, PRNU, hot and dead pixels, blooming into the neighbours, row noise and a
column fixed pattern, read noise, analog gain, black pedestal, ADC depth and clipping); and an ISP.
EVERY defect of the lens and the sensor SHALL have the ISP block that real cameras use to remove
it, and each block SHALL be bypassable and mis-tunable: black-level subtraction (the pedestal and
the dark mean, with a tuning error), the optical-black row/column correction, defect-pixel
correction, green equalisation, raw noise reduction by the sensor's noise model, RGB-IR
subtraction, lens shading and colour lens shading correction, white balance and digital gain,
demosaic (bilinear, Malvar–He–Cutler or edge-directed) under an assumed CFA phase, dewarp and
lateral-CA correction, highlight handling, the colour correction matrix, a tone curve, YUV denoise,
false-colour and purple-fringe suppression, sharpening, BT.601/709 YUV with 4:4:4/4:2:2/4:2:0
chroma and a decoder matrix. A `view` row SHALL show the picture after any stage (the scene, the
light through the lens, the pixel charge, the raw, the raw after each correction, the demosaic,
the lens correction, luma, chroma). The picker SHALL offer the presets phone at night, cheap
webcam, security camera at night (IR), cinema raw, wrong CFA phase and no IR-cut filter, each only
a set of the passport's rows. The camera SHALL film only the scene it has and its ISP SHALL read
only the sensor: the lens's focal length is fitted so that every ray of the sensor's border, in
every channel, lands inside the recorded frame, and the ISP's lens correction crops its output to
the region the sensor covers — no stage repeats an edge texel into a band.
Why: the owner's request I1 of kkt-26.11.01 and his rule of 2026-11-01 — an imitation of a
physical effect is not a simulation of it; and his review of the same day: "you added the noise but
not the denoiser … the PAIRED block of optics compensation simply does not exist … what is hidden
in real ISPs, the distortions and the ways to remove them, you had to put into the fx" (F-FX-010).
The temporal stages (3DNR with its trails, rolling shutter, mains flicker) need the slot's previous
output and are the change `temporal-barrier-3dnr`, not part of this requirement. The field rule:
the README footage (2026-11-03) showed "Phone at night" with a 30 px smear along the edges — the
dewarp read past the sensor, and a barrel lens past the frame (camera-isp-edges).

#### Scenario: A clean chain reproduces the input
- **WHEN** a smooth colour frame passes `camera_isp` on every layout and demosaic, with no temporal
  noise, no fixed pattern, no defects, optics flat, exposure below clipping, the calibrated WB and
  CCM, the sRGB curve, no denoise or sharpening and 4:4:4
- **THEN** away from the frame border a 2×2 Bayer gives the input within 1 LSB on average and 3 on
  every pixel, the other mosaics (a generic interpolation) within 2 on average and 12 on every
  pixel; flat patches are within 1 LSB on every layout
- **TEST** t1211 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: Shot noise variance grows with the signal
- **WHEN** flat grey frames of four levels are filmed with read noise 0 and no fixed pattern, tone,
  WB and CCM bypassed, and the raw DN of the G pixels is read
- **THEN** the DN variance divided by the mean signal in DN equals the conversion gain within 10 %
  at every level (variance ∝ signal), and with read noise σ the variance at a dark level is the
  shot variance plus σ²·g²
- **TEST** t1212 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: A wrong CFA phase swaps colours as predicted
- **WHEN** a red, a green and a blue patch are filmed by an RGGB sensor without noise, and the ISP
  assumes the phase shifted by one pixel in x and y (BGGR), then in x only (GRBG)
- **THEN** with the x-and-y shift red comes out blue and blue red while green stays green; with
  the x shift green comes out magenta (R and B high, G low) and red and blue come out green
- **TEST** t1213 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: An infrared leak tints as predicted
- **WHEN** a mid-grey frame is filmed with scene NIR, the IR-cut filter out and the visible WB
  calibration, on an RGGB sensor; then on an RGB-IR sensor with the ISP's IR subtraction on
- **THEN** the RGGB output's linear R, G and B are grey plus `qe_ir·T_c·IR/qe_c` each — R and B
  above G, the magenta cast — within 3 %; with the IR-cut filter in it is grey; the RGB-IR output
  with subtraction is neutral (its channels within 3 LSB of each other)
- **TEST** t1214 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The demosaic zipper on a known edge
- **WHEN** a vertical black/white edge is filmed by an RGGB sensor without noise and developed by
  the bilinear, Malvar–He–Cutler and edge-directed demosaics
- **THEN** the bilinear output alternates colour along the edge with a two-row period (the zipper)
  by more than 20 LSB between neighbouring rows, and the edge-directed output (green by gradients,
  red and blue by colour differences) alternates at most half as much
- **TEST** t1215 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The noise is deterministic, the fixed pattern is fixed
- **WHEN** a preset is rendered twice at the same timeline position, once at another, with and
  without the SIMD twin, and once at K=2
- **THEN** the same position gives the same bytes on every path; another position changes the
  temporal noise but not the hot pixels; the K=2 frame is the K=1 frame magnified by 2
- **TEST** t1216 (`src/plugins/fx-video-camera/tests/consequences.rs`), t1053

#### Scenario: The GPU graph agrees with the CPU kernel
- **WHEN** every preset and the defaults run on the GPU executor and on the CPU, on a frame with
  and without alpha
- **THEN** each pair is within `FrameTol::THRESHOLD` and the alpha is the source's
- **TEST** t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`), t1052

#### Scenario: Each layout is the camera's mosaic
- **WHEN** the layout tables are read
- **THEN** each has its period and colour fractions (Bayer ½ G, ¼ R, ¼ B; Quad Bayer the same in
  2×2 blocks; X-Trans 20/36 G, 8/36 R, 8/36 B; RGBW ½ W; RGB-IR ¼ IR, ½ G), and every preset sets
  only passport rows within their ranges
- **TEST** t1218 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The effect window is the pipeline
- **WHEN** the person adds the camera effect to a clip and opens its window
- **THEN** the window shows the blocks in the order of the chain, each stage with its bypass and
  its rows; every passport row has its place; switching a stage's bypass reaches the model and
  joins the window's one undo step
- **PATH** select a clip → the effect picker "+ effect…" → type "camera" → click "Camera ISP" (the
  window shows the blocks Source, Lens, Sensor, ISP: raw, ISP: demosaic, ISP: lens correction,
  ISP: colour, ISP: YUV; each correcting stage says which defect it removes) → turn the wheel down
  over the window to the ISP: raw block → untick "White balance" (the preview turns green)
- **TEST** t1219 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every defect has its correcting block
- **WHEN** a smooth pattern is filmed by a camera with an ideal lens, then with one defect at a time
  — distortion, lateral CA, vignetting, colour shading, Gr/Gb imbalance, a column fixed pattern, hot
  pixels — once with its ISP block calibrated and once with the block off
- **THEN** without the block the picture differs visibly from the ideal camera's; with the block the
  difference falls below half (for hot pixels: at most a quarter of the lit pixels remain); a
  geometric pair is compared with the ideal camera at the output's own paraxial magnification
  (crop/fit with the block, 1/fit without), since correcting distortion on a sensor no larger than
  the output costs field
- **TEST** t1220 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The view shows any stage
- **WHEN** a grey frame is shown at every value of `view`
- **THEN** the scene of an sRGB source is the input within 1 LSB; the raw is a mosaic in its filter
  colours (a red site red only, a green green only, a blue blue only); every raw-domain view
  differs from the output
- **TEST** t1221 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: The frame is unprocessed into the scene
- **WHEN** a frame developed with a saturation of 1.4 and a warm white balance is unprocessed with
  exactly those settings, and once without unprocessing
- **THEN** the unprocessed scene is the plain frame's scene within 2.5 LSB on average; without
  unprocessing it keeps the source's look (at least three times further)
- **TEST** t1222 (`src/plugins/fx-video-camera/tests/consequences.rs`)

#### Scenario: Raw NR follows the noise model; blooming spills
- **WHEN** a noisy two-level frame is developed with raw NR off and on; and one overexposed spot is
  filmed with blooming 0 and 1
- **THEN** raw NR at least halves the noise variance of the flat part and keeps the edge step within
  12 LSB; with blooming the raw of the spot's neighbour rises by more than 200 DN

#### Scenario: The frame's edges are picture, not smears
- **WHEN** a frame whose brightness is a sawtooth along each axis (rising from every edge inward)
  is filmed by every preset, the defaults and strong distortion and lateral CA with corrections
  off, weak, calibrated and doubled, on the CPU and on the GPU
- **THEN** the mean along the middle third of each edge changes by at least 1.5 LSB within every
  4 px of depth from 1 to 28 px — no band repeats the edge texel; with distortion and CA 0 the
  bytes are the old ones
- **TEST** t1430 (`src/libs/kt-gpu/tests/camera_isp.rs`), t328

### Requirement: A simulation of a device that records sound names its audio half
Every simulation video kind whose device records sound (`vhs_tape`, `datamosh`, `camera_isp`) SHALL
declare its audio pair, and its sound rows SHALL stand at the end of the passport, never read by the
picture: the picture keeps its bytes and a file without them keeps its text. A new simulation effect
of such a device is not done until its pair exists, or its note says why the device has no sound.
Why: kkt-26.11.03b D1 — "again the agents did not think that VHS Tape must have a paired audio filter,
Datamosh too"; the root CLAUDE.md rule written with it.

#### Scenario: Every built-in simulation of a sound-recording device is paired
- **WHEN** the built-in set is assembled
- **THEN** `vhs_tape` names `vhs_tape_sound`, `datamosh` names `datamosh_sound`, `camera_isp` names `camera_isp_sound`, and no declared pair is missing (the assembler refuses otherwise)
- **TEST** t1460 (`src/libs/kt-plugin-api/tests/pairs.rs`), the assembler's check in `kt_plugins_builtin::builtin`
