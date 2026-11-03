# Design

## Context

C2 of kkt-26.11.03 asks for a mechanism to accelerate the program's computations on whatever the
machine has (AVX, CUDA, OpenCL, Vulkan). Read in the code (2026-11-03):

- **GPU:** `kt-gpu` already compiles WGSL at run time through wgpu onto Vulkan, DX12, Metal and GL
  (fragment passes only — the GL backend on llvmpipe has no compute). Every picked video effect
  carries a WGSL text in its manifest (`Gpu::Wgsl`) or a pass graph.
- **CPU:** every effect has a scalar f64 kernel — the byte reference (invariant 11). AVX2 twins
  exist only where a barrier was measured slow: glow's additive finish (`fx-video-blur/src/simd.rs`),
  edges (`fx-video-stylize/src/simd.rs`), the camera rows (`fx-video-camera/src/simd.rs`), plus
  `kt-fx/src/video_simd.rs` and `kt-engine/src/video_simd.rs` for the host's resampling and
  composite. The switch is `kt_fx::simd::simd_level()`: Scalar or AVX2, nothing in between, nothing
  for ARM. The point-wise kinds (Color Grade, Posterize, Duotone, Vignette…) are scalar only.
- **Cores:** `std::thread::scope` appears in exactly two places (camera, tape). The rest of a frame
  runs on one thread; the export pipelining is a separate change (`export-throughput`).

## Goals / Non-Goals

**Goals:** measured evidence for the owner's decision — how far one text can replace three, what the
cores and the wider SIMD give, what the GPU side costs — on ONE effect, without changing a shipped byte.

**Non-Goals:** migrating any effect; a new dependency; touching the engine's planning or the export.

## Decisions

### D1. The single text is the common subset of Rust and WGSL, not a new language

`shared_kernel!` takes `fn`s whose tokens are valid Rust AND valid WGSL (`let`, `return`, `vec3f(…)`
constructors with all components, `.x/.y/.z/.w`, the operators, `select` instead of `if`/`var`) and
expands them twice into Rust (`cpu` with `f32` aliased to f64, `cpu_f32` with a real f32) plus
`stringify!` of the same tokens as the WGSL string.

Alternatives measured or rejected:
- **CubeCL** 0.11 (Rust `#[cube]` kernels → WGSL/CUDA/HIP/MSL/CPU): compute shaders only (our
  GL/llvmpipe path has none); `cubecl-wgpu` depends on `wgpu ^30` — ours today, but every eframe
  upgrade would then wait for CubeCL too; its CPU runtime is a run-time LLVM 23 JIT (`llvm-sys` and a
  fetched LLVM), so the CPU form is CubeCL's float pipeline, not our f64 reference.
- **rust-gpu** (Rust → SPIR-V): a pinned nightly toolchain and a separate shader crate build; the
  WGSL would go through naga's SPIR-V front end — a second compiler between the text and the GPU.
- **Slang / ISPC / Halide**: a second language and toolchain (C++/LLVM) in the build; ISPC and Halide
  CPU paths are fast but their float semantics are their own (Halide's `strict_float` aside), not our
  f64 reference.
- **wgsl-rs** 0.1.0-beta.6 (a Rust subset run on the CPU and emitted as WGSL): the same idea as a
  young third-party proc-macro; our ~330 lines need no dependency, and adopting it later stays open.
- **A WGSL interpreter on the CPU** (parse the WGSL with naga, run it): f32 only, and an interpreter
  is slower than the scalar kernel it replaces.

The subset's price: no `var`/`if` (a branch evaluates both sides), constructors spelled in full,
float literals with a point, helper names prefixed per kernel (every plugin's WGSL lands in one fused
module). Every builtin is exact or correctly rounded on both sides; nothing transcendental (that stays
in `kt_plugin_api::math`, and a GPU `sin` would not match it anyway — the tolerance carries it).

### D2. One loop, several SIMD copies, row bands on scoped threads

`run_pointwise` compiles one generic band loop under `#[target_feature]` for SSE4.1, AVX2 and
AVX-512 and picks the widest the CPU runs. Rust emits no FMA contraction and no reassociation without
an explicit intrinsic, so the copies are the same bytes by construction (t1500 holds it). SSE4.1 alone
matters: on the x86-64 baseline `floor`/`round` are libm calls, with SSE4.1 they are one instruction.
Threads: a frame is cut into bands of whole rows, one scoped thread per band (no pool: the prototype
measures the ceiling; a shared pool is the migration's work).

### D3. Measured against the shipped texts in the same harness, never through the engine

The GPU test builds the own pass exactly as `assembled_shaders` does and runs both WGSL texts in one
raw-wgpu harness; the engine is not touched, so the prototype cannot reach a shipped frame.

## Risks / Trade-offs

- The subset is narrow: a barrier with loops over taps, a stateful kernel or one reading neighbours
  does not fit as is — loops (`for`/`loop`) and array reads need a bigger subset or a proc-macro
  translator. Recorded in the note's memo as the main migration risk.
- The f64 CPU form keeps the reference but halves the SIMD width against f32: measured, the f32 form
  differs from the reference on ≈ 1 channel in 80 000 by 1 LSB — a reference change the owner would
  have to order (goldens regenerate).
