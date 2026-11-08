## ADDED Requirements

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
SHALL prewarm every device it composes on and wait for the worker before its first frame. The pipelines
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
