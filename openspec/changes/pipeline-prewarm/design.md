# Design: pipeline-prewarm

## Context

`docs/engine-audit.md` M8, row 6 of the order; found-defects 64 and 139. The evidence there was
"unverified"; this change measured it first (D8).

The instrument: `kt-cli first-frame [--size WxH] [--kind ID] [--rebuild]` — one headless engine at
the project size, every video effect of the registry at its defaults: the first frame against the
minimum of three warm ones. The excess of the first is a compile inside it, ours or the driver's.

Before (release, llvmpipe 20.1.2 on Vulkan, 1080p, 32 threads):

| | empty Mesa cache | Mesa cache warm |
|---|---|---|
| engine build (every pipeline created) | 1900 ms | 948 ms |
| a second engine, its own device | 905 ms | 732 ms |
| the first composite of all (no effect) | 10 499 ms | 29 ms |
| Camera ISP first frame / warm | 4600 / 89 ms | — |
| VHS Tape | 1234 / 64 ms | 90 / 64 ms |
| Event Grid | 598 / 7 ms | — |
| vhs | 358 / 5 ms | — |
| lens_distort (a geometry shape) | 111 / 15 ms | — |
| all effects: first-frame excess | 7170 ms | 66 ms |

So: creating a pipeline is the smaller part; a software rasterizer compiles its code at the first
DRAW (llvmpipe builds a fragment-shader variant then). The Mesa disk cache hides it from the second
run on, but every driver update, a fresh install, a new machine, and any device whose driver keeps
no disk cache pays it inside the frames a person watches.

## D1. The mechanism and the class

The mechanism: code for the GPU is compiled at its first use, and that first use is inside a frame
(or inside a UI frame), so the first frame of an effect waits for a compiler.

The walk — every site that creates a `ShaderModule`, `RenderPipeline` or `ComputePipeline`, and
every place whose driver compiles at first use:

| Site | When it compiled before | Verdict |
|---|---|---|
| `GpuFxEngine::build`: the 8 fixed pipes, every point-wise plugin's own pass, every barrier plugin's GPU passes (`plugin_barriers`), `display`, `geom_fused` | created at engine build, all of them; their CODE at the first draw on a software rasterizer | **CAUSE** of the 10.5 s first composite and of every effect's first-frame excess: warm draws (D2) |
| The display pass into the ambient glow (`glow:resample`, 2 s on llvmpipe in the audit) | the same `display` pipe; its first draw paid the compile | covered by warming `display` (D2) |
| `geom_pipeline` — a geometry shape's own pass (`geom_specialized`) | created INSIDE the frame that first met the shape (row 139), and drawn there | **CAUSE**: the interpreter runs the frame, the shape compiles and warms on the worker (D4) |
| The editor's engine (`init_video_gpu`) | built on the UI thread in the first `ui()` of every `KtApp` — at start and at every project open: 0.95–1.9 s of a frozen window on llvmpipe | **CAUSE**: built on a thread; the pipelines of the display device are kept and a project opened again takes them (D3) |
| `offline_engine()` (effect thumbnails, reel strips, posters, the slow-chain device) | built on its own thread already; its frames paid the first draws | prewarmed with the player's reach when it is the slow-chain device; a thumbnail job draws each effect once anyway |
| `export_engines()` (2–4 devices on a software adapter) | built at export start; the first frames of each device paid the first draws | the export prewarms the project's reach on each device before its first frame (D5); the time moves, the file is the same |
| The compute host (`compute.rs`): device | opened at the first job, inside the stream stage's frame (row 64) | **CAUSE**: opened by the prewarm when the project reaches a compute kernel (D6) |
| The compute host: `pipe()` | created inside the first job using the text, under the pipes lock | **CAUSE**: plugins declare their shaders, the prewarm prepares and dispatches each once (D6) |
| `compute::compile`, `compile_check` | tests only (t1650, t1651: FXC on the Windows runner) | not a frame path |
| egui-wgpu's own pipelines (the UI's renderer) | eframe creates them at start | not ours; the preview's texture registration creates no pipeline |
| Each new device recompiles everything (`cache: None` in every descriptor) | — | the disk tier where the device can (D7) |

## D2. A warm draw

One draw of each pipeline into a 6×6 target (one per format: the integer frame, the word format,
the display format), zero uniforms, 6×6 inputs, one encoder, then a wait for the device. When it
returns, the driver has compiled what the frame will run. 6×6 and not 1×1: a driver may key its code
variant by whether a texture's sides are powers of two, and a frame's are not. Zero uniforms make
every loop bound zero: the cheapest run. Measured (all 56 pipes, empty Mesa cache): the warm draws
take 17.4 s and leave a first-frame excess of 354 ms over all effects (from 7170), the rest being the
geometry shapes D4 removes. 17 s is why a prewarm warms what the project REACHES (D5), not
everything: a project without Camera ISP must not pay its 4.5 s.

The kit (bind groups, targets, a uniform) is made once per device and outside `created()`: a warm
thread runs beside frames, and t2023 counts what a frame makes.

## D3. Compiled pipelines belong to the device

`Compiled` holds the layouts, every pipeline, the warm kit, which pipelines are warm, the geometry
shapes and the device's pipeline cache. The editor's display device lives as
long as the process, and `KtApp` is rebuilt at every project open after the old one is gone, so its
engine is made by `new_kept`: the process keeps that device's `Compiled`, and a project opened again
builds its engine in microseconds, already warm. An ordinary engine (an export's devices, the
offline one) compiles its own, and its pipelines go with it (an export device holds 1.3 GB).

(A `wgpu::Device` compares by its id inside its instance: a device of another instance can equal
one of ours. So only `new_kept` looks the device up, and only the editor calls it, for its one display
device; every other engine compiles its own and drops them with it.)

The editor builds the engine on a thread (`engine-build`) and hands it to the player and the display
when it stands; until then the preview has no picture, as before the first frame, and the
performance panel says "no video FX".

## D4. Geometry shapes off the frame path

A geometry shape (`geom_specialized`) is a function of the program's step kinds and supersampling,
which follow from parameter values and frame sizes only the plan knows. Predicting them would mean
building plans for every moment of the project. Instead a frame that meets a shape it has no warm
pipeline for runs the interpreter (`geom_fused`, warm from D2): t1766 holds the two to the same bytes,
so the picture does not change when the shape's pipeline arrives. The shape goes to the device's warm
worker, which compiles it, warms it and publishes it; a later frame takes it. The interpreter is
slower per pixel (a loop the compiler cannot unroll on llvmpipe) for the frames until then: frames,
never a wait.

## D5. What a project reaches, and when

The player computes the effect ids of the project (clips, tracks, morphers, compositions, generator
kinds) when it takes a project, and hands the NEW ones (not yet asked of this executor) to
`VideoGpuExec::prewarm` and to `kt_plugin_api::compute::prewarm`. The first call also warms the core
(the fixed pipes, display, the interpreter). `prewarm` returns at once: the device's warm worker
(one thread per `Compiled`, started when work arrives, ended when the queue is empty) draws them.
Nothing runs at rest: a project whose ids did not change asks nothing; the worker is gone when done.

An id maps to its pipelines by the owners `plugin_pipe_names` already keeps (a point-wise plugin's
own pass, a barrier's passes). Transitions live in the composite pipe (one pipe, warm with the core);
geometry plugins are steps of the fused pass (D4).

The export prewarms its devices with the project's reach and waits (`settle`) before its first frame:
the export's time is the same work, but its first frames no longer stall the pipeline ring.

## D6. The compute host

A plugin names its compute shaders (`Plugin::compute_shaders`: the text and how many buffers it
reads and writes, which fix its layout). `compute::prewarm` opens the host on its own thread
(`compute-warm`) when one is installed and calls `ComputeHost::prepare` for each: the pipeline is
created and dispatched once with zero buffers (a software rasterizer compiles a compute variant at
its first dispatch too). A job that still has to compile a text counts it (`pipelines_in_frame`), a
dispatch of a pipeline never dispatched before counts a cold draw.

Generator kinds that draw on the compute device (gradient, corners, noise, rect, ellipse, line) name
their `pixel` text once; the frame function and the declaration read the same constant.

## D7. The disk tier

Where the device has `Features::PIPELINE_CACHE` (wgpu offers it on Vulkan), every pipeline of a
`Compiled` and of the compute host is created with one `PipelineCache` made from the file
`<data>/shader-cache/<key>.bin`. The key is a hash of `wgpu::util::pipeline_cache_key` (vendor and
device), the driver name and its version string, the wgpu version, and the hash of every shader text
the engine compiles; a changed driver or a changed shader is another file, and a stale one is never
read. The data is written after the warm worker empties its queue, only when it grew, atomically
(a temporary file and a rename); the folder keeps the four newest files and a file above 64 MB is not
written. A cache is speed only: Vulkan validates the header itself (`fallback: true` ignores a
foreign blob), and the frames are the same bytes with and without it (t2125). Where the feature is
missing (GL, the live check's llvmpipe, Metal, DX12) nothing changes.

The folder is set by the program (`kt_gpu::set_cache_dir`: the editor's data folder, kt-cli's);
tests set none, so no test writes into the tree or the home folder.

## D8. Measured after

(filled in stage 6)
