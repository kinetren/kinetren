# Proposal: pipeline-prewarm

## Why

`docs/engine-audit.md`, mechanism M8 ("allocation or resource creation per frame, compile in the
frame"), row 6 of its order: the first frame of an effect hitches on every machine because a GPU
pipeline is compiled inside it. found-defects 64 (the first Datamosh frame 6.6 s under load: the
compute device opens and its shaders compile inside the frame) and 139 (a new geometry shape
compiles its pipeline inside the frame). The owner asked for the performance of everything
(`docs/ktprompts/kkt-26.11.08.txt` H10–H17) and, on 2026-11-09, said "запускай compiled-curves и
pipeline-prewarm". Limits must be general or measured at run time, never tuned to one machine.

Measured before the change (`kt-cli first-frame`, release, llvmpipe on Vulkan, 1080p): with an empty
Mesa shader cache the very first composite took 10.5 s, Camera ISP's first frame 4.6 s, VHS Tape's
1.2 s, Event Grid's 0.6 s — 7.2 s of first-frame excess over all effects — while creating every
pipeline of the engine took 1.9 s. A software rasterizer compiles a pipeline's code at its first
DRAW. And the engine itself was built on the UI thread at every project open (0.95–1.9 s).

## What Changes

- The class walked: every place that creates a pipeline or shader module lazily, or whose driver
  compiles at first use (design D1).
- **Prewarm**: when a project opens and when an effect enters it, the pipelines its chains can reach
  are drawn once into a few-texel target (compute shaders: compiled and dispatched once) on a thread
  of their own; a frame never waits for that work, and until a pipeline is warm the frame behaves as
  today.
- **Geometry shapes never compile in a frame**: an unseen shape runs the interpreter (the same
  bytes, t1766) while its own pipeline compiles and warms beside.
- **The compute host prewarms**: a plugin declares its compute shaders; the device opens and they
  compile and dispatch once off the frame path.
- **Compiled pipelines are per device**: a second engine on the same display device (a project
  opened again) takes them as they are; the editor builds its engine off the UI thread.
- **Disk tier**: a wgpu `PipelineCache` where the device has `PIPELINE_CACHE` (Vulkan), in the data
  folder, keyed by the adapter, driver, wgpu version and shader text, bounded; same bytes with and
  without it.
- **A count of cold draws**: a pipeline drawn or dispatched in a frame before it was warmed. Zero
  after prewarm for every registered plugin's GPU kernel and the core nodes.

## Impact

- `kt-gpu`: `warm.rs`, `Compiled` per device, `pipeline_cache.rs`, counters `cold_draws`,
  `pipelines_in_frame`; `GpuFxEngine::{new_kept, prewarm, settle}`; the compute host's `prepare`.
- `kt-plugin-api`: `ComputeHost::prepare`, `compute::prewarm`, `compute::Shader`,
  `Plugin::compute_shaders`; generator kinds name their shader.
- `kt-engine`: `VideoGpuExec::prewarm`; the player prewarms what a project reaches.
- `kt-app`: the engine is built off the UI thread; `kt-cli first-frame`.
- Capabilities: `video-plan`, `plugins`.
