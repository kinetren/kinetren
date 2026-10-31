# Spec Delta

## MODIFIED Requirements

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
- **TEST** t1055 (`src/libs/kt-fx/tests/geo_fuse.rs`)
