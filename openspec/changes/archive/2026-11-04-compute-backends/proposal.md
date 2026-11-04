# Proposal

## Why

kkt-26.11.03 item C2 (`docs/ktprompts/kkt-26.11.03-index.md`; `docs/user-feedback.md`, "2026-11-03 —
the final render is slow; one way to accelerate the math"). The owner: "Как я понимаю, у нас
"математика" в каждом месте своя, и она не ускоренная. Нужно подумать как ее ускорять и при том, что
на машине может быть как AVX, так и cuda, так и opencl, так и vulkan - т.е. нужен механизм, который
позволит ускорять различные расчеты в программе. Наверное уже за нас всё придумано давно, и есть
что-то типо шейдеров которые умеют компилироваться. Может быть вообще другой подход выбрать."

Read in the code, today one effect is written up to three times — the scalar f64 kernel (the byte
reference), an optional AVX2 twin (only the barriers have one: blur/glow, edges, camera; the
point-wise colour effects have none) and a WGSL text for wgpu. The CPU path runs on one core
(threads only inside the camera and the tape). This is an ARCHITECTURE question the owner decides on
measured evidence; this change brings the evidence and does NOT migrate the program.

## What Changes

- A reconnaissance note `docs/features/plugins/kernels/compute-backends.md` (F-EXT-004):
  BlinkScript, Halide, ISPC, OpenCL/SYCL, CUDA, CubeCL, rust-gpu, Slang, naga; how Resolve,
  Premiere, After Effects, Blender Cycles and OFX hosts pick a backend; the verdict for us and a
  decision memo for the owner in Russian.
- A prototype, unused by the shipped program: `kt_plugin_api::shade` — `shared_kernel!` takes ONE
  kernel text in the common subset of Rust and WGSL and gives its f64 CPU form (the byte reference),
  an f32 CPU form and the WGSL string; `run_pointwise` runs a point-wise kernel over bands of rows on
  scoped threads, the same loop compiled for SSE2/SSE4.1/AVX2/AVX-512 and picked at run time.
- Color Grade written once (`fx-video-color/src/single_source.rs`): t1500 holds its CPU form byte for
  byte to the shipped kernel at every SIMD level and thread count; t1501 compiles its WGSL on the GPU
  within the kind's `FrameTol::BILINEAR`; two `--ignored` benchmarks give the note's numbers.
- No shipped output changes: the plugin keeps `grade` and `color_grade.wgsl`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `plugins`: a requirement recording the single-text kernel contract the prototype proves (the CPU
  form of one text is the byte reference at every SIMD level and thread count; its WGSL form is held
  to the kind's tolerance) — the decision record the migration, if the owner orders it, builds on.

## Impact

- `src/libs/kt-plugin-api/src/shade.rs` (new, no dependency added), `lib.rs` (`pub mod shade`).
- `src/plugins/fx-video-color/src/single_source.rs` (new), `lib.rs` (`pub mod single_source`),
  `tests/single_source.rs` (t1500 + bench).
- `src/libs/kt-gpu/tests/single_source.rs` (t1501 + bench), `Cargo.toml` (dev-dependency
  `fx-video-color`, allowed by t975).
- Docs: the note, `docs/features/README.md` coverage line, the registry row C2.
