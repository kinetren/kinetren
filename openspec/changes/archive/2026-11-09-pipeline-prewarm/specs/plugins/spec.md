## ADDED Requirements

### Requirement: A plugin names its compute shaders; the host prepares them before the first frame
A plugin whose kernels run shaders on the compute host SHALL name every one of them in
`Plugin::compute_shaders` (the text and how many buffers a step reads and writes); a generator kind
drawn with `shader_frame` names its `pixel` text with `Kind::with_shader`, from the same constant.
`compute::prewarm(shaders)` SHALL return at once, open the host on a thread of its own and call
`ComputeHost::prepare` for each shader not asked before; `prepare` compiles the pipeline and dispatches
it once over zero buffers. A job that compiles a text SHALL count it in `kt_gpu::pipelines_in_frame()`
and a first dispatch of a pipeline never dispatched in `kt_gpu::cold_draws()`.
Why: found-defects 64 — the first Datamosh frame opened the compute device and compiled its shaders
inside the frame (6.6 s under load); measured on llvmpipe, the residual shader alone compiles 4.1 s
with an empty Mesa cache.

#### Scenario: A prepared Datamosh compiles nothing in its first frame
- **WHEN** the host prepares the shaders Datamosh names, then the first intra frame is encoded, a predicted one and a decode run
- **THEN** no pipeline is created and none dispatched cold
- **TEST** t2122 (`src/plugins/fx-video-codec/tests/compute_prewarm.rs`)

#### Scenario: Every generator kind names its shader
- **WHEN** the host prepares what every generator kind of both families names, then each kind draws its first frame at its defaults
- **THEN** no frame creates a pipeline or dispatches one cold (before: the leader named none)
- **TEST** t2123 (`src/libs/kt-gpu/tests/compute_prewarm.rs`)

#### Scenario: The prewarm returns at once and prepares each shader once
- **WHEN** two shaders are prewarmed twice while the host is still opening, then one of them again beside a new one
- **THEN** both calls return before the host opens; each shader is prepared once, and the third call prepares only the new one
- **TEST** t2124 (`src/libs/kt-plugin-api/tests/compute_prewarm.rs`)
