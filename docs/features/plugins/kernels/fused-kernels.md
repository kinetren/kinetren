---
id: F-EXT-005
title: Fused and specialized kernels — JIT of an effect chain
status: researched
priority: B
cost: L
category: plugins/kernels
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, nuke-17]
we_have: partial
crates: [kt-plugin-api, kt-fx, kt-gpu, kt-engine]
invariants: [7, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-EXT-004]
trackers: []
services: []
checked: 2026-11-08
---

# F-EXT-005 — Fused and specialized kernels — JIT of an effect chain

## Essence

An effect chain on an event runs as one program generated at run time, instead of one GPU pass per
effect. Point-wise effects are folded into one shader, so intermediate values stay in registers and
never round-trip through a texture. Discrete settings (a mode, a tap count) are compiled in as
constants. Compiled pipelines are kept between runs, so the first use of an effect does not hitch.
The person sees the same picture, sooner, and no stall the first time an effect appears.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | OpenFX plug-ins render one effect at a time; no documented fusion |
| Premiere | 26 | Mercury Playback Engine | a GPU kernel per effect; frames stay on the GPU only while the whole chain is GPU, otherwise everything before a CPU effect renders on the CPU too; no cross-effect fusion documented |
| Resolve | 21 | DCTL / Fusion tools | one kernel per tool (DCTL compiled to CUDA/OpenCL/Metal per backend); the only merging is transform concatenation (a run of transforms resamples once), as in Nuke |
| Final Cut Pro | 12 | Core Image (the OS layer under it) | kernel concatenation: filters build an immutable recipe; at render time colour and warp kernels of the graph are combined into ONE program; a general kernel (neighbourhood reads) is rendered to an intermediate first |
| After Effects | 26 | GPU-accelerated effects (Smart Render GPU) | a kernel per effect per framework (CUDA/OpenCL/Metal/DirectX); "Collapse Transformations" for precomps is the only concatenation |
| Nuke | 17 | BlinkScript | a kernel per node, translated at run time to C++/SIMD or OpenCL; nodes are not fused; transforms concatenate |

**Outside the core.** The mechanism is defined outside video editors: by an image language
(Halide), shader-graph compilers of game engines, emulators that hide compile hitches, and audio
DSP compilers. The verdict needs their numbers and their failure modes.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Halide | 19 | algorithm / schedule | stages are fused per tile (`compute_at`), recomputing the tile border; without fusion pipelines are memory-bound |
| Unreal Engine | 5 | material graph, PSO precaching | a graph becomes one HLSL shader; permutations multiply compile time; possible pipelines are compiled in the background, a draw whose pipeline is not ready is skipped or uses a default |
| Dolphin | 2506 | ubershaders, hybrid mode | an interpreter shader draws while the specialized one compiles in the background, then is swapped in |
| Faust | 2.79 | `compute()` / libfaust | a whole DSP diagram compiles into one per-sample loop (C++/LLVM JIT/WASM); `-vec` deliberately splits it again so non-recursive parts vectorize |
| wgpu | 27 | `override` constants, `PipelineCache` | overrides are substituted by naga before the backend text is emitted (a full compile per value); the pipeline cache is implemented on Vulkan only |

## The trick behind it

- **Fusion is decided by the kernel's class, not by the effect.** Core Image fuses only two classes:
  - a *colour* kernel (one sample in, one colour out);
  - a *warp* kernel (output coordinate → input coordinate, one image, with a region of interest).

  A *general* kernel (it samples anywhere: blur, glow, a simulation's neighbourhood) cuts the run and
  is written to a texture. So the plugin's declared class, not a heuristic, says what fuses. Apple's
  advice is to split a filter into warp + colour parts precisely to stay fusable.
- **A run-time constant is not free in wgpu.** A WGSL `override` looks like a Vulkan specialization
  constant. wgpu-hal, however, replaces it in the naga IR before emitting SPIR-V/MSL/HLSL. Each new
  value is a full translation plus a driver compile, so specializing on a continuous or automated
  value would compile every frame. Only discrete values qualify.
- **The hitch is hidden by drawing something else first, never by waiting.** UE5 and Dolphin draw
  with a generic program while the specialized pipeline compiles. For us this is safe only for the
  preview, and only for frames that are never stored. The same frame from two different programs
  would differ in rounding, and invariant 11 plus the kept-composite cache need the same bytes every
  time.

## Pros

- A run of k point-wise effects reads and writes the frame once instead of k times. On an ARM or
  integrated GPU, where a 4K frame round trip is a large part of the memory bandwidth, this is the
  dominant cost.
- k − 1 fewer dispatches, barriers and bind groups per frame (found-defects row 138 is their CPU side).
- No 8-bit quantization between fused effects: the intermediate stays f32.
- A disk pipeline cache plus prewarm at project open removes first-use stalls. Found-defects row 64
  (6.6 s first Datamosh frame under load) and row 139 (a geometry shape compiled inside a frame) are
  this.

## Cons

- The fused output differs from the unfused one by the rounding the textures used to apply (≤ 1 LSB
  per former boundary on 8-bit textures). Every stored-frame reference of a fused chain moves.
- Generated shaders have no human source. A compile error or a wrong pixel must be mapped back to the
  effect, and the generated text dumped.
- A fused shader takes the register budget of its heaviest stage. Long runs lower occupancy or
  spill, so run length needs a cap measured on real cards.
- Very large generated shaders trip compilers: FXC on Windows (row 97 already lives at that edge),
  and slow, fragile mobile drivers.
- It does nothing for multi-pass simulations dominated by neighbourhood reads and ALU: Camera ISP
  (12 passes, 92–97 ms a frame on llvmpipe) and VHS Tape (14 passes, 65–72 ms) are our two heaviest
  costs (gpu-pass-cost D2).

## What users say

NO QUOTES: this pass read vendor documentation, engineering write-ups and source code (Apple WWDC
2014/515 and 2021/10159, Photoroom's renderer article, the Igalia write-up on specialization
constants, Dolphin's ubershader article, the HexoSynth `synfx-dsp-jit` docs, wgpu-hal 30 source).
No user forum thread asking for or complaining about effect-chain fusion in an editor was searched.
The engineering sources say the expected: Photoroom calls a generic (non-fusable) kernel
"dramatically more expensive", and a Cranelift DSP log found per-block dispatch "isn't the
bottleneck". The owner's own claim is the demand: «И может быть задуматься JIT математики и
вычислений?» (kkt-26.11.08 line 18).

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/

### What already exists

- **Fusion of geometry, and point-wise taps absorbed into it.** `plan_chain`
  (`src/libs/kt-fx/src/plan.rs:278`) segments a chain. Geometry runs plus absorbed point-wise taps
  execute as one `geom_fused.wgsl` pass, specialized per program shape (`geom_specialized`,
  `src/libs/kt-gpu/src/engine.rs:1036`, render-speed-no-card D4: 14 → 7 ms on the warp shape).
- **Barriers.** Barrier plugins declare their GPU pass graph (`plugin-api-barriers`).
- **No pipeline cache.** Every `create_*_pipeline` passes `cache: None` (`engine.rs:1250`, `:1281`,
  `:1333`, `:2680`; `src/libs/kt-gpu/src/compute.rs:147`, `:301`). Each new device (the offline
  device, each export device) compiles everything again.
- **Hand specialization is already the practice.** gpu-pass-cost hoisted per-pass constants out of
  per-pixel code (Camera ISP 115–129 → 92–97 ms).
- **Audio.** It is compiled ahead of time with an AVX2 second copy (`cpu::fast!`, audio-fx-cost: VHS
  sound 40 → 22 reference kernels on that path).

### What is missing

- A *colour* / *warp* / *general* class in the plugin passport, and a pure-function form of a
  point-wise kernel the generator can call.
- A generator that writes one WGSL pass for a run: load, the calls, store.
- The rule that offline paths and stored composites wait for the fused pipeline, while the preview
  may show an unfused frame it never stores.
- Prewarm of the project's known pipelines at open, and wgpu `PipelineCache` on Vulkan.

### What hinders

- **Measurement.** Our only per-pass numbers are from llvmpipe (gpu-pass-cost D2: point-wise
  effects 2–3 ms each at 1080p, composite 1.1 ms a layer). On a card these passes are short and
  bandwidth-bound, and his journal's `GPU` record (found-defects row 108) has not come yet. The gain
  of fusion on his machine is unknown.
- **Stored frames.** The references of fused chains move: tolerances `FrameTol`, t181, t183.
- **Invariant 7.** One composition path must hold: the generator must live inside the single
  executor, not beside it.

### Which invariants it touches

- 7: one composition path; the generator is part of the GPU executor of `frame_plan`.
- 11: video bytes equal between runs on one device. This means no timing-dependent choice between the
  fused and unfused program for anything stored or exported.

## Estimate

| What | Answer |
|---|---|
| Scope | L for fusion (a round); S for prewarm + pipeline cache; specialization stays per effect |
| Where the main work is | kt-plugin-api (kernel class, pure function), kt-gpu (generator, cache), kt-fx `plan_chain` |
| Project format | does not touch |
| Tests that will have to be written | fused equals unfused within the kind's tolerance; no in-frame pipeline creation after prewarm (t2023's style); offline never uses a fallback |
| Depends on | F-EXT-004 |
| Breaks compatibility | plugin API extended (point-wise form); stored frames of fused chains re-recorded |

## Verdict

Take the pipeline prewarm and the Vulkan pipeline cache now: cheap, and they invisibly remove rows 64
and 139. Take fused point-wise runs later, as generated plain WGSL on the Core Image class model, once
a real card's `GPU` record shows point-wise passes are a real share of a frame. Specialize only by
discrete values. Reject audio JIT: audio is 0.1–4.4 % of real time, a block dispatch is not the cost,
and a second implementation endangers byte-exact audio. Reject CPU SIMD JIT: the CPU video path was
deleted by the owner's decision. Analysis and order: `docs/engine-audit.md` §3–§4.

## Sources

Read 2026-11-08; core-editor rows from vendor documentation and forums, not checked live.

- https://developer.apple.com/videos/play/wwdc2021/10159 ; https://nonstrict.eu/wwdcindex/wwdc2014/515/ ; https://developer.apple.com/documentation/coreimage/cirenderinfo
- https://photoroom.com/tech/building-cross-platform-image-renderer
- https://github.com/CommandPost/ResolveCafe/blob/main/docs/developers/dctl.md ; https://forum.blackmagicdesign.com/viewtopic.php?p=525680
- https://community.adobe.com/t5/premiere-pro-discussions/faq-mercury-playback-engine-cuda-opencl-metal-and-what-it-all-means/m-p/3179781
- https://blog.adobe.com/en/publish/2016/07/10/after-effects-cc-2015-3-in-depth-gpu-accelerated-effects
- https://learn.foundry.com/nuke/content/reference_guide/other_nodes/blinkscript.html
- https://people.csail.mit.edu/jrk/halide-pldi13.pdf ; https://dl.acm.org/doi/10.1145/3306346.3322967
- https://blogs.igalia.com/itoral/2018/03/ ; https://nonstrict.eu/wwdcindex/wwdc2016/605/
- https://docs.rs/crate/naga/29.0.3/source/src/back/pipeline_constants.rs ; https://docs.rs/wgpu/latest/wgpu/struct.Features.html
- https://dev.epicgames.com/documentation/unreal-engine/pso-precaching-for-unreal-engine ; https://dolphin-emu.org/blog/2017/07/30/ubershaders/
- https://faustdoc.grame.fr/manual/options/ ; https://github.com/cmajor-lang/cmajor ; https://docs.rs/synfx-dsp-jit ; https://rhizome-lab.github.io/resin/design/jit-optimization-log.html
- https://dawn.googlesource.com/tint/+/b293f51f83cab41b45f334414c44e6d50fe64ba0
- Our measurements: `openspec/changes/archive/2026-11-08-gpu-pass-cost/design.md` (D2), `openspec/changes/archive/2026-11-08-audio-fx-cost/design.md`, `docs/engine-audit.md` §0.
