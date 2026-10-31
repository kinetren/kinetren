# Spec Delta

## MODIFIED Requirements

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
