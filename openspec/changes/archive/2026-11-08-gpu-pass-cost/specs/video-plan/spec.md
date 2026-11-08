## ADDED Requirements

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

#### Scenario: A Datamosh frame reuses the compute host's buffers
- **WHEN** a warm Datamosh encoder job and decoder job run three times
- **THEN** no buffer or pipeline was created
- **TEST** t2024 (`src/plugins/fx-video-codec/tests/compute_reuse.rs`)
