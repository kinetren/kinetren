# Plugins — why it is this way

## What hurt and for whom

Every effect, transition and generator was a variant of a closed enum in kt-model (46 FX, 33
transitions, 14 generators), with a `tag()` arm, a serde form, a kt-fx kernel and a kt-gpu pipe
written by hand. An edit of one effect touched kt-model, so it rebuilt every crate above it (R.68:
a 61 s cascade), and an effect from outside could not exist at all. The owner accepted the plugin
contract in R.59 (F-EXT-003), asked for the internal API and crates first (2026-10-27), and then
for "ALL the effects out — a mechanism as flexible and extensible as possible" (2026-10-30).

## Decisions and why

The full text, alternatives and measurements are in the archived change
`openspec/changes/archive/2026-10-31-plugin-api-stage1/design.md`.

- **Ids are today's tags; a missing id is a kept slot (D3).** A plugin's id is the string the
  variant's `tag()` returned, and its passport lists its rows in the variant's old field order.
  A `.kt` therefore reads the same text before and after a kind moved (t973, t1048, frozen
  oracles inside the tests). A kind this build lacks reads as `Missing` and writes back what it
  read. A numeric index (it moves on insertion), a wrapper object (it changes every file's lines,
  against R.70) and a refusal to open were all rejected. Ids are unique within a FAMILY: the
  generator `checker` is not the transition `checker` (t1049).
- **The refusal at registration is split by medium (D4).** F-EXT-003's "a plugin that declares
  state or a tail is refused" holds for video, where post-FX memoization would lie. For audio it
  is narrowed: audio has no post-FX cache, and every built-in reverb, delay and limiter keeps
  state the host owns (invariant 3). Still refused: any video state, tail or look-ahead; a kernel
  that is not callable from several threads; an audio look-ahead reaching `MIN_PREROLL_FRAMES`; a
  built-in point-wise kernel without WGSL; a geometry kernel that offers WGSL (t972).
- **The GPU side is assembled from the registry; geometry stays the host's primitives (D5,
  D11.2).**
  - A point-wise plugin's one WGSL tap is pasted both into its own pass and into the fused pass.
  - A geometry plugin returns a program of the host's primitives. Its one open primitive is a warp
    function pasted into both loops. Jacobian supersampling, the f32 parity and the exact lone
    path stay in one hand-checked file.
  - A cache key hashes the plugin id, never a dispatch number.
- **Each reason a kind was core became a contract (D11).**
  - A positioned tap (vignette).
  - The geometry program (warps, corner_pin, mirror, event_grid, pan3d).
  - A host-reference row with a role (the compressor's sidechain key, pan3d's camera).
  - A time-warp declaration (Slowed+Reverb).
  - The transition kernel (D12): `pack` to eight f32 once, one CPU text, one WGSL function.
  - The generator kernel (D13): a frame, a tone and gate, and a declared `gen:` path token table
    read by ONE codec in kt-model.

  The core keeps only what a person does not pick: the Geometry lead node, CamMotion, LayerMix,
  the crossfade and the title. The title's rows are text, a font family and a layer order, and
  its presets and R.53–R.58 migration are kt-model's.
- **The order of external hosts: wasm before dlopen/OpenFX (D10).** It revises F-EXT-003's
  "built-in → dlopen → OFX". A wasm module has only the imports we give it, so "no clock, no
  I/O" becomes a property; a trap is caught where a segfault is not; one binary serves Linux and
  Windows. dlopen keeps its value for OpenFX, which is C ABI by definition.
- **The leaf rebuild is measured, not assumed (t977).** An edit of a plugin kernel recompiles only
  its family, the assembler, kt-app and kt-cli (`src/scripts/plugin-rebuild-check.py check`; red
  for an edit in kt-fx). The TEST build does not get cheaper: the assembler is a dev-dependency of
  seven core crates, whose ~100 integration test binaries relink (archived tasks 1.1 and 7.2).

- **A barrier is a CPU kernel, its twin behind the host's one switch, and a declared GPU graph**
  (`plugin-api-barriers`, 2026-11-01; the archived change's design.md D1–D7).
  - `BarrierKernel::frame` is the byte reference. Its AVX2 branch runs only when `BarrierCtx::simd`,
    which the host sets from `KT_SIMD` and the CPU, so the scalar reference stays ONE switch away
    (R.32). A separate `frame_simd` entry was dropped as a second door with the same arguments.
  - The GPU side is a graph of the host's nodes (`Resample`, `Premul`, `Unpremul`, `Save`) and the
    plugin's own fragment passes over `common.wgsl`. A plugin never issues encoder commands: the
    arena, the readback cache and device-loss poisoning stay the host's. A graph of host nodes
    alone is legal (blur). The output must be at the frame size, and a trailing `Save` is refused —
    the saved list would release the texture the output still reads.
  - The shared raster code (the resample with its AVX2 twin, the bilinear sample,
    premultiplication) is reached through `RasterHost`, not copied into the API. The geometry
    resample and a barrier's mip stay one text.
  - Noise and greenscreen are point-wise plugins with a barrier kernel: their tap inside a fused
    series, their integer kernel at a series edge, exactly where the core kinds ran. The K rule
    stays the planner's (t1055).
  - Moving the barriers exposed three "is this the same kind" checks that compared the enum
    discriminant, which every plugin slot shares:
    - `FxInstance::lerp` blended blur's `radius` into glow's;
    - `chain_fingerprint` kept an EQ's processor when a filter replaced it in the same slot
      (t1057, audio-render);
    - two test guards had gone blind the same way.
    All compare the kind's tag now.

## Deferred

- The wasm host (stage 2), then dlopen and the OpenFX bridge (stage 3).
- A plugin that ships UI: kind-specific widgets (the EQ graph, colour rows, camera handles) stay in
  kt-app, looked up by plugin id.
- The cost of the test build after a plugin edit (fewer test binaries per crate, or the
  registry-wide tests gathered where the set is needed).

## A barrier with its own grid and a deck of numbers (vhs-tape-simulation, archived 2026-11-01)

- `Node::PassAt` (a plugin pass at its own size) and the value forms `frame_values`/`gpu_values`
  were added for the VHS tape simulation, whose signal grid is the standard's (a 640-wide preview
  has fewer texels than the grid) and whose ~30 physical quantities do not fit eight cooked
  numbers. Both are additive: a resample-like check for the size, and defaults that cook, so every
  earlier barrier kept its bytes and graph (t1187). Rejected: packing the grid into frame-sized
  textures (the layout would change with the preview size) and packing several quantities into one
  f64 (a quantisation the passport does not show).
- The first barrier declaring `AlphaRel::Destroys` found that the GPU's strength mix used the
  chain's END alpha flag for both sides and left the fold untouched: under a strength the dry
  side was forced opaque and the mixed layer declared opaque (t1052, 86 % of samples off). The mix
  now carries the fold at its points and sets it to dry ∨ wet, as the CPU's two frames do.

## The stream class (datamosh-codec-path, archived 2026-11-01)

- **A temporal kind declares a HISTORY, not state.** F-FX-007 named the way through the purity rule —
  read the past SOURCE, keep nothing — and `VideoClass::Stream` is that declaration: the
  `StreamKernel` answers which stream positions frame N reads (`needs`, given the memo) and renders N
  as a pure function of them and of its rows AT each position (`rows_at`, the curves applied by the
  host). `state`, `tail` and `lookahead` stay refused for video.
- **The region of interest is part of the contract** (`region_ident`, `with_region`; the owner's
  "programmatic handles" for an object detector): the host names WHERE a stream kernel acts, the
  kernel decides what that means. Today no host part gives a region.
- **The memo is the host's (`StreamMemo`):** the preview shares one process memo so the prefetch's
  "what is still needed" and the composer's results meet; each export owns a private one so a
  preview cannot evict what the export reads next. Keys are addresses (media, pts, size, the source's
  revision), so the needs are answered without reading a frame — two test streams that shared
  addresses shared keys, which is why the engine's ident carries the source and its revision.
- **Refused at registration (t1240):** the class without a stream kernel, a stream kernel on another
  class, any GPU side on a stream plugin (D4 of the archived change).

## The values door (camera-isp-simulation and vhs-tape-simulation, 2026-11-01)

`Cooked` is eight numbers; a simulation has dozens of rows (camera_isp ~85). Packing them into bit
fields would quantize every automated row, so `BarrierKernel::frame_values`/`gpu_values` receive the
slot's values in passport order, defaulting to `frame`/`gpu` over `cook` (a cooked barrier keeps its
bytes — t1187). The host calls only these forms: kt-fx `apply_barrier`, the kt-gpu barrier op, the
registry's graph check (which therefore sees a graph broken only at a ninth row's top — t1210). The
two branches that needed it at once wrote the same text (`ebc58282`, `831cccf5`), so it merged
without a conflict.

## One kernel text — the compute-backends prototype (archived 2026-11-04)

C2 of kkt-26.11.03 asked for one mechanism to accelerate the math on AVX, CUDA, OpenCL or Vulkan.
The GPU side already is "shaders that compile" (WGSL through wgpu onto Vulkan/DX12/Metal/GL, every
vendor); what was missing is the cores and one text instead of three. `kt_plugin_api::shade` proves
the mechanism on Color Grade without shipping it: the common subset of Rust and WGSL is one text
(`shared_kernel!`: `cpu` with `f32` = f64, `cpu_f32`, `WGSL` by `stringify!`), and one band loop is
compiled for SSE2/SSE4.1/AVX2/AVX-512 and run on scoped threads. The bytes stay because rustc emits
no FMA contraction and no reassociation and the subset's builtins are exact (t1500); the WGSL keeps
the kind's tolerance class (t1501). Rejected: CubeCL (compute-only, an LLVM JIT as its CPU form),
rust-gpu (a pinned nightly), Halide/ISPC/Slang (a second language and toolchain, their own float
rules), CUDA/OpenCL (no new hardware over Vulkan/DX12/Metal, a third text and tolerance class per
effect). Measurements, the migration estimate and the owner's questions: F-EXT-004
(`docs/features/plugins/kernels/compute-backends.md`).

## The CPU level of an audio kernel (audio-fx-cost, 2026-11-08)

- `cpu::run_fast` compiles the kernel's block a second time inside a `#[target_feature(enable =
  "avx2")]` function; the closure is `#[inline(always)]` (`fast!`), or the body stays out of line with
  the baseline code — the first try showed exactly that in the disassembly. AVX2 without FMA: Rust
  neither contracts `a * b + c` nor reorders float sums, so the copy gives the same bytes (t1981).
- Hand-written SIMD was rejected: a second text to keep bit-equal by hand per kernel, where the
  compiler's second compile of the same text is equal by construction and costs one line per kernel.
- The AAC codec, the noise suppressor's frame and pitch's grains through the same door measured no
  gain and were not kept. `math::powf_neg_half` is `powf(x, −0.5)` to the bit, checked on all
  2 139 095 040 positive finite floats (t1982).
