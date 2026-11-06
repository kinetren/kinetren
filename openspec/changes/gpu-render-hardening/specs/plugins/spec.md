## ADDED Requirements

### Requirement: A test model answers to its shaders
A simulation whose physics tests read an f32 model SHALL have a test running the model and the GPU side
by side at every preset, by the mean of the difference, so that a pass edited alone is red.
Why: tests that read a model say nothing of a shader that left it.

#### Scenario: The tape
- **WHEN** one frame runs through the model and through the shaders, at the defaults and each preset
- **THEN** the mean difference is under 0.02 (a clogged head: 0.9)
- **TEST** t1640 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The camera
- **WHEN** one frame runs through the model and through the shaders, at the defaults and each preset
- **THEN** the mean difference is under 0.002
- **TEST** t1641 (`src/plugins/fx-video-camera/tests/consequences.rs`)

### Requirement: Every shader compiles on D3D12 in CI
The Windows job SHALL compile every engine pipeline and every compute shader on the D3D12 adapter
through FXC, and SHALL run no frame on WARP.
Why: FXC refuses what naga and Vulkan pass, and no other job reaches it.

#### Scenario: The engine's pipelines
- **WHEN** the engine builds on a D3D12 adapter, WARP included, in compile-only mode
- **THEN** every shader compiles; with no D3D12 the test skips
- **TEST** t1650 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The compute shaders
- **WHEN** the codec's shaders compile on a D3D12 adapter
- **THEN** none is refused; with no D3D12 the test skips
- **TEST** t1651 (`src/plugins/fx-video-codec/tests/codec.rs`)
