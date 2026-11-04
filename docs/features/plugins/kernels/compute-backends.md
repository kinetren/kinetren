---
id: F-EXT-004
title: Compute backends — write the math once, run it on whatever the machine has
status: researched
priority: B
cost: XL
category: plugins/kernels
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, nuke-17, blender-5.2]
we_have: partial
crates: [kt-plugin-api, kt-fx, kt-gpu, kt-engine]
invariants: [7, 11]
ui_rules: []
round: null
point: null
tests: [t1500, t1501, t1502, t1503, t1504, t1505]
depends_on: [F-EXT-003]
trackers: []
services: []
checked: 2026-11-03
---

# F-EXT-004 — Compute backends — write the math once, run it on whatever the machine has

## Essence

The owner asked (C2 of `kkt-26.11.03`) for one mechanism that accelerates the program's math on
whatever the machine has — AVX, CUDA, OpenCL or Vulkan — "something like shaders that compile". Seen
from the editor's chair, it means three things: an effect renders as fast as the machine allows,
the result does not depend on which machine rendered it, and a new effect does not cost three texts.
The subject of this note is how other programs answer that, and what follows for us. It is
reconnaissance plus one measured prototype, not a migration.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | GPU acceleration of video processing (Preferences → Video) | one switch for the whole render: a GPU (OpenCL/CUDA through the vendor driver) or Off; the effects carry no per-API choice of their own [NOT VERIFIED] |
| Adobe Premiere | 26 | Mercury Playback Engine — GPU Acceleration (CUDA / OpenCL on Windows, Metal on macOS) | the renderer is a project setting; the CUDA/OpenCL selector appears only when both APIs are present. macOS dropped OpenCL and CUDA in 2021. **"Software Only" was removed in v25.2** (announced 2025-01-07): there is no CPU renderer any more |
| DaVinci Resolve | 21 | Preferences → Memory and GPU → GPU processing mode: Auto / CUDA / OpenCL / Metal | the guidance is NVIDIA → CUDA, AMD → OpenCL; without a usable GPU API it refuses to start ("Unsupported GPU Processing Mode"); a CPU path for image processing was not found [NOT VERIFIED as an official statement] |
| Final Cut Pro | 12 | Metal only | one API on one platform; nothing to choose [NOT VERIFIED] |
| After Effects | 26 | Mercury GPU Acceleration + Multi-Frame Rendering | two independent axes: GPU-accelerated effects run on the GPU, the rest on the CPU; MFR (2021) renders several FRAMES at once on CPU cores (up to half the cores, 8 in the first beta; in GPU mode also bounded by VRAM), effects not yet "MFR-safe" are excluded |

**Outside the core.** The core editors pick an API; the mechanism "one kernel text → several backends" exists only in compositing and rendering software and in compiler projects.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Nuke | 17 | BlinkScript (Foundry Blink) | a per-pixel C++-like kernel compiled at run time "into normal C++ or SIMD code for the CPU, or OpenCL for the GPU" (Metal on macOS); the CPU SIMD mode is experimental and "might fail for some valid kernels"; on GPU out-of-memory a frame falls back to the CPU |
| Blender | 5.2 | Cycles devices: CPU, CUDA, OptiX, HIP, oneAPI, Metal | ONE restricted C++ kernel with per-target macros (`ccl_device`, `ccl_global`…), a thin wrapper per device in `kernel/device/*`; two CPU builds (SSE4.2 and AVX2+FMA) picked at run time by `system_cpu_support_avx2()`; OpenCL removed in 2021 |
| OpenFX | 1.5 (`ofxGPURender.h`) | `kOfxImageEffectProp{Cuda,Metal,OpenCL}RenderSupported` / `…Enabled` | the plug-in and the host each declare which GPU APIs they support; the host decides PER RENDER call and hands device pointers plus a stream/queue, or CPU pointers — so a GPU plug-in keeps a CPU path; a GL failure code makes the host retry on the CPU |

Compiler and runtime projects (checked 2026-11-03; versions from their release pages):

| Project | Version | Reach | Determinism (can the CPU stay a byte reference?) | Cost in our build | Licence |
|---|---|---|---|---|---|
| Halide | 22.1.0 (2026-10-31) | x86 SSE…AVX-512, NEON/SVE; CUDA, OpenCL, Metal, D3D12, Vulkan; WebGPU "work-in-progress" | `strict_float` on LLVM targets only; GPU schedules promise nothing | a C++ generator at build time, a Halide/LLVM distribution (hundreds of MB), FFI; no Rust crate | MIT |
| ISPC | 1.32 (2026-10-13) | CPU only: SSE2…AVX-512, AVX10.2, NEON; multi-target object + dispatcher | FMA on by default (`--opt=disable-fma`), its own stdlib for transcendental functions | the `ispc` binary at build time or prebuilt objects (`ispc_rt` 2.0.4); a second language | BSD-3 |
| OpenCL / PoCL / SYCL | OpenCL 3.0; PoCL 7.x; AdaptiveCpp 25.10, DPC++ | NVIDIA (second-class), AMD, Intel, Mesa Rusticl; Apple deprecated (2018); PoCL = OpenCL on the CPU | strict by default, vendor ulp limits for built-ins; nothing bit-exact across devices | a C kernel language + driver ICDs; SYCL a clang-based C++ compiler | Khronos / various |
| CUDA (cudarc 0.19.10; Rust-CUDA; cuda-oxide 0.2) | CUDA 13 | NVIDIA only | nvcc contracts FMA by default | dynamic CUDA libraries; Rust kernels need a pinned nightly | proprietary SDK; crates MIT/Apache |
| CubeCL | 0.11.3 (2026-10-22) | WGSL/SPIR-V via wgpu, CUDA, HIP, MSL, CPU via a run-time LLVM 23 JIT | the CPU form is CubeCL's LLVM pipeline, not our f64 reference | many cubecl-* crates, `llvm-sys` + a fetched LLVM for the CPU; `cubecl-wgpu` depends on `wgpu ^30` (matches ours today); **compute shaders only** | MIT/Apache |
| rust-gpu | 0.11.0 (2026-10-13) | SPIR-V; `spirv-unknown-naga-wgsl` → WGSL (naga 30); fragment shaders supported | the same Rust compiles natively on stable — the CPU form can be ours | the shader crate on a PINNED nightly (nightly-2026-08-15), `cargo-gpu`, a slow codegen backend, pin churn per release | MIT/Apache |
| wgsl-rs | 0.1.0-beta.6 (2026-10-19) | a Rust subset run on the CPU and emitted as WGSL (validated by naga) — the idea of our prototype as a crate | CPU form is plain Rust | one young proc-macro crate | MIT |
| Slang | v2026.20 (2026-10-17) | HLSL, SPIR-V, CUDA, MSL, WGSL (experimental), CPU C++ (experimental, compute-only, via slang-llvm) | releases still change CPU float semantics | a C++ compiler library (tens of MB), an HLSL-like language, FFI | Apache-2.0 + LLVM exception |
| naga | 30.0.1 | back ends spv/msl/glsl/hlsl/wgsl — **no CPU or interpreter target**; no maintained WGSL interpreter found | — | already in our tree (inside wgpu) | MIT/Apache |

## The trick behind it

**First. Nobody keeps one RESULT — they keep one SOURCE.** Cycles, the strongest single-source
precedent, compiles its CPU kernels with `-ffp-contract=on -freciprocal-math` (and `/fp:fast` on
MSVC), so its own SSE4.2 and AVX2 builds can disagree in the last bit — a path tracer only needs
statistical equivalence. Blink's documentation once promised identical CPU/GPU output on NVIDIA
"with accuracy limitations"; the current page no longer says it. Premiere solved the disagreement by
deleting the CPU renderer. Our contract — the CPU is a byte reference, the GPU a declared tolerance
class — is stricter than every precedent found, and it decides the choice: a system whose CPU path is
generated by its own compiler pipeline (CubeCL's LLVM JIT, Halide, ISPC with its FMA default, Slang's
CPU target) cannot be our reference without an audit, while a Rust text compiled by rustc can,
because rustc never contracts `a·b + c` and never reassociates without an explicit intrinsic.

**Second. The API is not the axis; the CORE COUNT is.** The editors that "support CUDA, OpenCL and
Metal" expose a choice of driver API — the same GPUs, reached three ways. After Effects scales on a
second, independent axis: whole frames rendered in parallel on CPU cores (Multi-Frame Rendering).
On our machine, one text compiled for AVX2 and cut into bands of rows went from 31 ms to 3.1 ms per
4K frame from 1 to 32 threads, while the GPU (here llvmpipe, itself a 32-thread SIMD executor)
needed 5.3 ms for the pass alone and 13.5 ms with the upload and readback a CPU-side caller pays.
For a point-wise effect, the readback costs more than the math.

**Third. WGSL and Rust already share a subset.** `fn f(v: vec4f, n: f32) -> vec4f { let x = …;
return …; }` parses in both languages once the Rust side has `vec4f` as a type AND a constructor
function (types and functions live in different namespaces in Rust) and `f32` aliased to f64. So
one text gives the f64 CPU reference and the f32 GPU kernel with no compiler of our own and no new
toolchain — the trick behind wgsl-rs and our 330-line prototype.

## Pros

- One text per point-wise kernel: the two copies that drifted apart (the hand-written WGSL of Color Grade added `128 + br·255` as one term, the CPU in two) cannot drift, because there is nothing to mirror.
- The SIMD levels come free and keep the bytes: the same loop compiled for SSE4.1, AVX2, AVX-512 (and NEON on ARM) — no twin to write, no twin to test against the scalar one (t1500 holds all levels at once).
- The CPU path becomes a real backend, not only a fallback: 35× the shipped scalar Color Grade at 4K on 32 threads, faster than llvmpipe's GPU pass and well below a GPU round trip.
- No new dependency, no nightly, no C++/LLVM toolchain: the GPU side stays wgpu (fragment passes, llvmpipe works), the reach stays Vulkan/DX12/Metal/GL.

## Cons

- The subset is narrow: no `var`, no `if` (a `select` evaluates both sides), no loops, no arrays. Point-wise kinds fit; barriers with taps or neighbourhoods do not without a translator (a proc-macro that reads the Rust tree and prints WGSL — what CubeCL and rust-gpu do at much larger cost).
- f64 on the CPU halves the SIMD width against f32. An f32 reference would change ≈ 1 channel in 80 000 by 1 LSB — the goldens regenerate.
- ≈ 28 KB of machine code per kernel for four SIMD copies (≈ 1 MB for every video kind).
- CUDA/OpenCL stay out: no hardware we do not reach already, and a third text plus a third tolerance class per effect; OpenCL is deprecated on Apple and was dropped by Blender for driver bugs.

## What users say

> "driver bugs, and stalled OpenCL standard has made maintenance too difficult"
> — Brecht Van Lommel (Blender), on removing OpenCL from Cycles, 2021-04-23, https://code.blender.org/2021/04/cycles-x/

> "Software rendering can cause poor performance and rendering in 8-bit"
> — Adobe community manager, announcing the removal of Premiere's software renderer, 2025-01-07, https://community.adobe.com/announcements-732/now-released-software-rendering-option-removed-311771

> "when using the GPU, Premiere always blends in linear light"
> — Walter Soyka, quoted on the Adobe forum about GPU vs software renders differing, 2019-11-21, https://community.adobe.com/t5/premiere-pro-discussions/gpu-acceleration-vs-software-only-changes-with-opacity-quality/m-p/10754064

> "we still use fast math … the speedup we get here is too big to ignore"
> — comment in Blender's `intern/cycles/CMakeLists.txt` (MSVC branch), main as of 2026-11-03, https://projects.blender.org/blender/blender/raw/branch/main/intern/cycles/CMakeLists.txt

## How it fits Kinotrain

CAPABILITY: openspec/specs/plugins/

### What already exists

- **"Shaders that compile" are already here.** `src/libs/kt-gpu/src/engine.rs` compiles WGSL at run
  time through wgpu 30 onto Vulkan (Linux, Windows, Android), DX12 (Windows), Metal (macOS) and GL
  (fallback, incl. llvmpipe). Every picked video effect carries its WGSL in its manifest
  (`Gpu::Wgsl` for a point-wise tap, a pass graph for a barrier); `assembled_shaders` pastes them
  into one fused pass. NVIDIA, AMD, Intel and Apple GPUs are all reached through these four APIs —
  the same reach CUDA + OpenCL + Metal would give, from one text.
- **The CPU path is the byte reference** (invariant 11): an f64 scalar kernel per effect, no FMA, no
  reassociation, `kt_plugin_api::math` instead of the platform libm. The GPU is held to it by
  `FrameTol` classes (t181).
- **SIMD — only where a barrier was measured slow:** AVX2 twins in `src/plugins/fx-video-blur/src/simd.rs`
  (glow's finish), `src/plugins/fx-video-stylize/src/simd.rs` (edges), `src/plugins/fx-video-camera/src/simd.rs`,
  `src/libs/kt-fx/src/video_simd.rs` and `src/libs/kt-engine/src/video_simd.rs` (host resampling and
  composite); audio: one AVX+FMA sinc in `src/libs/kt-fx/src/audio/sinc.rs`. The switch
  (`src/libs/kt-fx/src/simd.rs`) knows Scalar and AVX2 only — no SSE4.1, no AVX-512, no NEON for an
  ARM machine. **The point-wise kinds (Color Grade, Posterize, Duotone, Vignette, …) are scalar
  only** — the brief "every effect has a SIMD twin" is wrong for them.
- **Cores:** `std::thread::scope` in two places (`src/plugins/fx-video-camera/src/cpu.rs`,
  `src/plugins/fx-video-tape/src/sim.rs`). Everything else in a frame runs on one thread.

### The prototype (change `compute-backends`, 2026-11-03)

`src/libs/kt-plugin-api/src/shade.rs`: `shared_kernel!` takes ONE text written in the common subset
of Rust and WGSL and gives three forms — `cpu` (the text's `f32` is f64: the byte reference),
`cpu_f32` (a real f32) and `WGSL` (the same tokens, `stringify!`). `run_pointwise` runs a point-wise
kernel over bands of rows on scoped threads, with one loop compiled for SSE2/SSE4.1/AVX2/AVX-512 and
picked at run time. Color Grade written once: `src/plugins/fx-video-color/src/single_source.rs` (30
lines replace `grade` + `color_grade.wgsl`). The shipped plugin is untouched.

Measured 2026-11-03 on the build machine: Intel i9-13900KF in a VM (32 threads, AVX2, **no
AVX-512**), release profile, a frame crossing every byte value, parameters (0.12, 0.35, 1.6, 60°),
best of 8–15 runs, the frame copy subtracted. The VM has **no real GPU** (QEMU VGA), so the "GPU" is
llvmpipe — Mesa's Vulkan on the CPU, itself a 32-thread SIMD executor; real-GPU numbers need the
owner's machine (the command is in the benchmark's doc line).

| Path (Color Grade, ms per frame) | 1080p | 4K |
|---|---|---|
| shipped scalar (`grade` through `frame_with`) — today | 26.7 | 108.4 |
| single text, baseline SSE2, 1 thread | 22.1 | 88.6 |
| single text, SSE4.1, 1 thread | 10.2 | 42.2 |
| single text, AVX2, 1 thread | 7.9 | 31.1 |
| single text, AVX2, 4 threads | 2.6 | 9.3 |
| single text, AVX2, 16 threads | 1.3 | 4.6 |
| single text, AVX2, 32 threads | 1.0 | 3.1 |
| llvmpipe, hand-written WGSL: pass only / upload+pass+readback | 1.51 / 3.81 | 5.27 / 13.46 |
| llvmpipe, single-text WGSL: pass only / upload+pass+readback | 1.57 / 4.14 | 5.69 / 14.56 |

Bytes: the single text's CPU form equals the shipped kernel **byte for byte** at every SIMD level
and band count (t1500, six parameter sets). Its real-f32 form differs from the reference on 104 of
8 294 400 channels at 1080p (396 of 33 177 600 at 4K), by 1 LSB. On the GPU both WGSL texts hold
`FrameTol::BILINEAR` (t1501): hand-written 140 / single text 175 channels of 8.3 M differ from the
CPU by 1 LSB at 1080p; the two GPU texts differ from each other on 65 channels by 1 — the
hand-written mirror added `128 + br·255` as one term, the CPU (and the single text) adds them in turn.

Cost: no dependency added; `fx-video-color` compiles in ≈ 0.5 s with or without the module (within
noise); its rlib grows 24 KB. Per effect the four SIMD copies of the loop are ≈ 28 KB of machine code
(SSE4.1 11.4 KB, AVX2 6.4 KB, AVX-512 5.0 KB, the dispatch 4.7 KB) against 0.9 KB of the scalar loop —
≈ 1 MB for every video kind. The shipped binaries do not contain the prototype (nothing calls it).

### What is missing

- A **shared CPU executor**: bands of rows on a pool for every point-wise and barrier kind, not
  `thread::scope` in two plugins. On this machine it is the larger half of the gain (AVX2 1 thread →
  32 threads: 31 → 3.1 ms at 4K).
- **One text per kernel.** Today a point-wise kind is two texts (f64 Rust + WGSL) kept equal by
  hand and by t181; a barrier with a twin is three.
- **SSE4.1 as a level.** `floor`/`round` are libm calls on the x86-64 baseline; SSE4.1 alone halves
  the scalar kernel (22 → 10 ms at 1080p) without touching a byte. Every x86-64 CPU since 2008
  (Penryn) / 2011 (Bulldozer) has it.
- **ARM.** No NEON level; an ARM Linux or Windows machine runs the scalar code. The single text
  compiled with NEON enabled would need no new kernel code.

### What hinders

- **The subset is narrow.** It fits point-wise kinds (no `var`, no `if`, no loops). A barrier
  (blur taps, edges' neighbourhood, the tape's filters) needs loops and array reads: either a bigger
  subset with a translator (a proc-macro reading the Rust syntax tree and printing WGSL — what
  CubeCL does), or barriers keep two texts.
- **The GPU is fragment passes, not compute** (`src/libs/kt-gpu/CLAUDE.md`): egui-wgpu on GL gives
  WebGL2 limits, so compute cannot run on THAT device. It is not a limit of the machine: llvmpipe on
  Vulkan runs compute (`src/libs/kt-gpu/src/compute.rs`), and the first version of this note
  was wrong to read it as one.
- **wgpu is pinned to eframe's major** — a library bringing its own wgpu (CubeCL) means two majors in
  one process, incompatible types, the preview unable to share the device.
- **f64 halves the SIMD width.** An f32 reference would double it but regenerates the goldens
  (t26/t11/t67) — a reference change only the owner orders.

### Which invariants it touches

7 (one composition path; CPU the byte reference, GPU within `FrameTol`) and 11 (determinism). The
prototype keeps both: the CPU bytes do not move, the GPU class does not move. CUDA or OpenCL as a
third backend would add a third text and a third tolerance class to hold against the CPU.

## Estimate

| What | Answer |
|---|---|
| Scope | XL — two rounds if the owner orders the full migration (see the memo) |
| Where the main work is | kt-plugin-api (subset, executor), every `src/plugins/fx-video-*` crate, kt-gpu (assembly unchanged) |
| Project format | does not touch |
| Tests that will have to be written | per kind: single text == the former CPU bytes at every level/thread count; WGSL within its class (t181 already loops over the registry) |
| Depends on | F-EXT-003 (the plugin contract) |
| Breaks compatibility | no (bytes kept); an f32 reference would regenerate goldens |

## Verdict

Take the mechanism, not a framework: (1) a shared multi-core CPU executor (bands of rows, byte-identical) and
SSE4.1/AVX2/AVX-512/NEON copies of ONE loop instead of hand twins; (2) one text per point-wise kernel in the
Rust∩WGSL subset (prototype `kt_plugin_api::shade`). Do NOT add CUDA or OpenCL: wgpu already reaches every
GPU vendor through Vulkan/DX12/Metal, and each extra API is a third text and a third tolerance class. Barriers
keep their two texts until a translator is worth its cost. The migration itself is the owner's decision (memo below).

## Sources

Read 2026-11-03; core-editor rows from vendor help and forums, not checked live; the outside-core and project rows from vendor documentation, release pages and source code.

- https://learn.foundry.com/nuke/Content/reference_guide/other_nodes/blinkscript.html ; https://learn.foundry.com/nuke/11.1/content/reference_guide/other_nodes/blinkscript.html
- https://code.blender.org/2021/04/cycles-x/ ; https://projects.blender.org/blender/blender/raw/branch/main/intern/cycles/CMakeLists.txt ; https://projects.blender.org/blender/blender/raw/branch/main/intern/cycles/device/cpu/kernel.cpp
- https://openfx.readthedocs.io/en/main/Reference/api/file/ofxGPURender_8h.html
- https://community.adobe.com/announcements-732/now-released-software-rendering-option-removed-311771 ; https://community.adobe.com/questions-534/multi-frame-rendering-is-here-aka-the-multithreading-you-ve-been-asking-for-314552
- https://forum.blackmagicdesign.com/viewtopic.php?p=1148722
- https://github.com/halide/Halide/releases ; https://halide-lang.org/docs/WebGPU.html
- https://phoronix.com/news/Intel-ISPC-1.32-Released ; https://crates.io/crates/ispc_rt
- https://www.phoronix.com/news/PoCL-7.0-Released ; https://github.com/AdaptiveCpp/AdaptiveCpp/releases
- https://crates.io/crates/cudarc ; https://rust-gpu.github.io/blog/2025/03/18/rust-cuda-update
- https://github.com/tracel-ai/cubecl/releases ; https://crates.io/api/v1/crates/cubecl-wgpu/0.11.3/dependencies ; https://crates.io/api/v1/crates/cubecl-llvm/0.11.3/dependencies
- https://github.com/Rust-GPU/rust-gpu/releases ; https://docs.rs/crate/wgsl-rs/latest
- https://docs.rs/naga/latest/naga/back/index.html
- https://shader-slang.org/slang/target-compatibility ; https://github.com/shader-slang/slang/releases
- Our measurements: `src/plugins/fx-video-color/tests/single_source.rs` (`bench_color_grade_cpu`), `src/libs/kt-gpu/tests/single_source.rs` (`bench_color_grade_gpu`), change `compute-backends`.

## Решение для владельца (memo)

Ваш запрос (C2, 03.11.2026): «у нас "математика" в каждом месте своя, и она не ускоренная … на
машине может быть как AVX, так и cuda, так и opencl, так и vulkan … есть что-то типо шейдеров которые
умеют компилироваться».

**Что есть сейчас.** «Шейдеры, которые компилируются» у нас уже есть: видеоэффекты на видеокарте
идут через wgpu, и один WGSL-текст компилируется при запуске под Vulkan, DX12, Metal или GL. Через
них доступны все видеокарты: NVIDIA, AMD, Intel, Apple. Вы правы в другом:
- на процессоре каждый эффект написан отдельно (эталон f64, а у тяжёлых ещё AVX2-копия);
- у точечных эффектов (Цветокоррекция, Постеризация, Дуотон, Виньетка) AVX2 нет вообще;
- кадр считается на ОДНОМ ядре из 32: потоки есть только в «Камере» и «Плёнке».

Итого эффект — это до трёх текстов, и машина загружена на 1/32.

**Что измерено** (прототип на «Цветокоррекции», кадр 4K, i9-13900KF; настоящей видеокарты на машине
сборки нет):

| Вариант | мс на кадр 4K |
|---|---|
| сейчас (скалярный эталон, одно ядро) | 108 |
| тот же единый текст, AVX2, 1 ядро | 31 |
| единый текст, AVX2, 32 потока | 3,1 |
| «видеокарта» llvmpipe: только проход / с загрузкой и выгрузкой | 5,3 / 13,5 |

Байты те же, что у сегодняшнего эталона, на любом наборе инструкций и при любом числе потоков (тест
t1500). WGSL-форма того же текста укладывается в допуск эффекта на видеокарте (t1501).

**Рекомендация.** Брать механизм, а не чужую систему: один текст ядра плюс общий многоядерный
исполнитель. CUDA и OpenCL не добавлять:
- новых видеокарт они не дают, все и так доступны через Vulkan/DX12/Metal;
- каждый такой API — это третий текст каждого эффекта и третий допуск;
- Apple от OpenCL отказалась, Blender выкинул его из-за ошибок драйверов.

Готовые системы не подходят:
- CubeCL — только compute-шейдеры (не работают на нашем запасном GL-пути) и тяжёлый LLVM для процессора;
- Halide, ISPC, Slang — второй язык и C++/LLVM в сборке;
- rust-gpu — ночная сборка компилятора.

Ни одна из них не держит процессорный путь байт-в-байт: у всех свой компилятор со своими правилами
округления.

**Во что обойдётся миграция** (оценка рабочего времени агента; эффектов сейчас: 24 видео, из них 6
точечных, 14 барьерных, 4 геометрических и 1 потоковый, плюс 32 перехода и 18 аудио):

1. **Этап А — многоядерный исполнитель и уровни SSE4.1/AVX2/AVX-512 для ВСЕХ видеоэффектов без
   переписывания ядер.** Около 1,5 недель.
   - Байты не меняются по построению: пиксель не зависит от соседних полос.
   - Даёт основную часть ускорения: на этой машине 10–35× для точечных эффектов.
   - Работает и в превью, и в финальном рендере. Это дополняет `export-throughput`, который
     распараллеливает этапы экспорта, а не математику.
   - Риск низкий.
2. **Этап Б — единый текст для 6 точечных эффектов и 32 переходов.** Около 2 недель. Переходам
   нужно расширить подмножество: `if`/`var` через процедурный макрос.
   - Риск средний: переходы обязаны совпасть по байтам с нынешним процессорным эталоном. Каждый
     держит свой тест.
3. **Этап В — барьеры (размытие, контуры, VHS…).** 4–6 недель: нужен переводчик циклов и массивов
   Rust→WGSL, а ручные AVX2-копии местами быстрее автоматических.
   - Риск высокий. Предлагаю решать после А и Б, по замерам.
4. Звук в этот механизм не входит: аудиоцепочка последовательна по состоянию (фильтры с памятью).
   Ускорять её — отдельная тема: параллельно по дорожкам и событиям.

**Вопросы, которые можете решить только вы:**

1. **Этап А — делать?** Ни один байт не меняется, ускорение на процессоре в разы.
   - «Да» — следующая волна.
   - «Нет» — процессор остаётся одноядерным запасным путём.
2. **Эталон f64 или f32?** Сейчас эталон f64.
   - f32 вдвое расширяет SIMD и ближе к видеокарте, но меняет ≈ 1 канал из 80 000 на 1 единицу. Все
     эталонные кадры (golden) перегенерируются.
   - «Оставить f64» — ничего не ломается, скорость чуть ниже.
3. **Подмножество языка для единого текста.** Ветвление записывается как `select` (обе ветви
   вычисляются), без циклов. Годится ли такой «язык ядер» для этапа Б, или сразу делать полный
   переводчик (дороже, см. этап В)?
4. **CUDA/OpenCL — подтвердить «нет»?** Исключение — если у вас есть машина, где Vulkan/DX12
   недоступен, а CUDA есть. Тогда назовите её.
5. **ARM (Apple silicon, Linux/Windows на ARM) — целевая платформа?** Если да, единый текст получит
   NEON без нового кода, но нужна сборка и CI под ARM.
6. **Замер на вашей видеокарте.** Здесь настоящей нет. Команда (из каталога `src`):
   `cargo test --release -p kt-gpu --test single_source -- --ignored --nocapture`. Она покажет,
   сколько стоит проход и выгрузка кадра на вашем железе против 3 мс процессора.

## Ответ владельца и пересмотр (04.11.2026)

Владелец: «Речь в моем запросе не шла только про процессорный рендер … я не верю что невозможно взять
условный opencl а лучше vulkan и на нем не писать всякие вещи где мы например AVX применяем»; «похер на
эталон»; «"ограниченный язык ядер" - а нам что нужно какой-то язык еще придумать?»; «да ARM нужен».

**Он прав, memo выше отвечало на слишком узкий вопрос.** Его держало требование побайтного эталона на
процессоре. Без него общий подмножественный язык и процессорная копия не нужны: ядро пишется обычным
compute-шейдером WGSL, с циклами, массивами и `if`. Утверждение про «compute не работает на llvmpipe»
было ошибкой: не работает только устройство интерфейса на GL.

Проверка (проба кодера вошла в программу как `src/plugins/fx-video-codec/src/wgsl/`; `compute_vhs_cost.rs`, release, 1080p, Vulkan через
llvmpipe, настоящей видеокарты здесь нет, на машине шла чужая работа, нагрузка ≈ 25; при нагрузке ≈ 48 кодер давал 78/70 мс):

| Что | Сейчас, CPU | Vulkan compute на эмуляции |
|---|---|---|
| Кодер Datamosh, P-кадр | 67 мс (один поток) | 34 мс (с загрузкой и выгрузкой) |
| Качество кодера, яркость PSNR | 46,01 дБ | 46,01 дБ |
| VHS Tape | 95 мс (полосы строк) | 108 мс (текущий граф из 14 фрагментных проходов) |

Вместо этапов А/Б/В: compute-узел в контракте плагина и своё Vulkan-устройство; затем Datamosh (у него
нет GPU-пути вообще); затем тяжёлые эффекты и удаление их CPU- и AVX-копий (816 строк, все только
x86-64). Прототип `shade`/`single_source` при этом удаляется. 
Второй ответ владельца (04.11.2026): «а если мы удалим - мы даже через llvm не сможем работать на
текущей машине»; «и видео и аудио можно удалить/обновить эталон».

- **Машина без видеокарты работает.** Все замеры выше сделаны именно так: Vulkan через LLVM (llvmpipe,
  пакет `mesa-vulkan-drivers`). Прежняя формулировка «тяжёлые эффекты там не посчитаются» верна только
  там, где нет ни одного Vulkan/DX12/Metal: Linux без этого пакета и Windows без драйвера (WARP на
  CI работал минутами и падал, поэтому отключён). Поставку своего программного Vulkan не проверяли.
- **Эталоны видео и звука можно удалять или обновлять.** Звук при этом остаётся на процессоре: снятие
  эталона разрешает менять его байты, а не переносит его на видеокарту.

Решение владельца: «да, удаляем» (04.11.2026). Этап 1 сделан в change `compute-kernels`: дверь compute в
контракте плагина, отдельное устройство, кодер Datamosh на шейдерах, его процессорный текст удалён.

Цена удаления процессорной сборки кадра, ещё не показанная владельцу при решении: экспорт витрины на этой
машине без видеокарты 95,6 с на процессоре против 133,9 с через llvmpipe, по замеру `export-throughput`
(`openspec/specs/export/design.md`). Причина там же: llvmpipe рисует по одному кадру через одну очередь.
Этот этап не начат, он ждёт отдельного ответа.

Замер итогового пути в программе (release, 1080p, та же картинка, Vulkan через llvmpipe, нагрузка ≈ 20):
P-кадр 46 мс против 67 мс прежнего кодера. В пробе было 34 мс: итоговый путь заново создаёт буферы на
каждый кадр и читает обратно весь остаток, а не только восстановленный кадр. Первый кадр после запуска
занял 6,6 с: открытие устройства и компиляция двух шейдеров на llvmpipe; пробе это не было видно,
потому что её таймер стартовал после компиляции.
