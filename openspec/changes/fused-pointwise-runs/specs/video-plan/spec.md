## ADDED Requirements

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
SHALL run as their own passes. A warm shape SHALL be tried against its own passes on interleaved
frames, timed by the pass timer; when the fused pass is slower per pixel than the sum of its parts
(minima of the tries) it SHALL be split into two halves, each tried again. On a device without
timestamps a warm shape SHALL run fused. The player SHALL hand the run shapes a project reaches to
the executor's `prewarm_runs` with its effect ids.

The fused and the own-pass forms SHALL give frames within 1 LSB per former boundary (the owner's
agreement, H16); on the reference software adapter they are byte-equal, so the measured switch never
moves a stored or exported frame.
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
