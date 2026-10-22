# Spec Delta

## ADDED Requirements

### Requirement: The "Geometry" node — leading, mandatory, movable
Each video event SHALL have exactly one LEADING "Geometry" node in its chain (the conform rule "follow the project" or its own, crop in fractions of the source, position in fractions of the canvas,
zoom, rotation, anchor); only the leading node carries the rule. Deleting the leading node SHALL
be rejected with a reason; reordering along the chain SHALL be ordinary; a "Geometry" node without a
rule SHALL be addable from the catalog any number of times. Effects BEFORE the leading node SHALL
run in the source's space (a buffer of decode size), AFTER — on the canvas; Pan3d and
CamMotion remain effects and by default stand above the conformed frame. The `Pan` kind SHALL
be absent from the registry; a leading node with defaults SHALL be an identity and share the
identity hash with the previous identity kinds. The order inside the node — crop, (the place for the mask),
placement — is fixed, so that a mask does not rewrite the node.
Why: in Vegas, Pan/Crop is the first node of the chain and draggable ("effects applied before
or after any event panning and cropping"); the owner asked for "not worse, but cooler"; the dead combo of three Pan modes is exactly the extra widget of rule 9.

#### Scenario: The leading node cannot be deleted, is moved, is added again
- **WHEN** the user deletes the leading node, moves it after Blur, adds a second "Geometry" node, and `FxInstance::all()` is checked against the golden table
- **THEN** the deletion is refused with a reason, the reordering and the second node pass, the second node has no rule, `Pan` is absent from the registry, the node with defaults gives the identity hash
- **TEST** t716 (`src/libs/kt-model/tests/fx_chain.rs`), t328 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: Before the node — source space, after — canvas
- **WHEN** on a vertical source, the chains Blur → Geometry and Geometry → Blur; a 90° rotation of the vertical source in 16:9
- **THEN** the frames differ (blurring the margin versus blurring the material), both backends are within tolerance; the rotated source fills the canvas without margins
- **TEST** t717 (`src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`)

## MODIFIED Requirements

### Requirement: The video effect registry is closed by a golden cycle
Each video kind from the shared registry `FxInstance::all()` SHALL have a row in the
golden table of default templates, and the table SHALL cover the registry in both directions:
a new kind does not build until its row is entered. Each kind MUST declare
its class for the scheduler (pointwise / barrier / geo) — an undeclared kind
is caught by a panic when run through the barrier path. The default of any new
parameter MUST give the previous behavior byte for byte: goldens are not
regenerated when parameters are added. The defaults of "identity" kinds
(Geometry, Pan 3D, Color Grade, Corner Pin, Drop Shadow on an opaque frame) legitimately
share one identity hash; the kind `Pan` is removed from the registry, and its row is absent from the table.
Why: "a new variant compiles and silently does nothing" — a forgotten arm of the
classification became a barrier and broke the geo-series; the completeness lock makes this
a build error rather than a finding of the user.

#### Scenario: A new kind will not build without a row
- **WHEN** a video kind is added to the registry without a row in the golden table or with a forgotten class
- **THEN** the golden cycle fails at the completeness lock (or a debug panic of the barrier path), while the hashes of all previous defaults stand
- **TEST** t328 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: The ten original kernels and their properties
- **WHEN** each of the first ten kernels is applied to a test frame
- **THEN** the hashes match the reference; mirror is an involution, blur preserves the mean and reduces the variance, greenscreen zeroes the alpha of the key corner, noise is deterministic by position, the grid tiles are identical, the leading "Geometry" node with defaults is an identity
- **TEST** t26 (`src/libs/kt-fx/tests/video_fx.rs`), t26 (`src/libs/kt-engine/tests/reverse.rs`) — the chain is applied in the single composite path, the cache stores the pre-FX frames (capability `video-plan`)

#### Scenario: A new parameter changes the picture, the default is identity
- **WHEN** an effect has gained a parameter (radius_y, thickness, noise size/color, glow threshold/tint, VHS scan_step/jitter, key spill)
- **THEN** each non-zero value meaningfully changes the frame, and the default gives the bytes of the previous kernel; the check runs over ALL channels, including those the kernel did not previously touch
- **TEST** t228 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: A grid larger than the frame does not overflow
- **WHEN** EventGrid is asked for more columns/rows than the frame has pixels
- **THEN** the grid is drawn from zero without centering, the frame size is an integer, the result is deterministic and mirrors the GPU guard
- **TEST** t299 (`src/libs/kt-fx/tests/video_fx.rs`)

#### Scenario: The buffer pool does not change the bytes
- **WHEN** the chain is executed with a "dirty" reused buffer pool
- **THEN** the result is byte-identical to a fresh run for each kind and for the chain Mirror+Grid+Glow+VHS
- **TEST** t68 (`src/libs/kt-fx/tests/video_fx.rs`)
