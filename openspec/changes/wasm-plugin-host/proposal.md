# Proposal: wasm-plugin-host

> IMPLEMENTATION-READY (spec-engine, revision 2, 2026-11-25, orch `fc60a5d9`; written 2026-11-20). Registry `v26.10-index.md` P1–P6 (no). Note F-EXT-003 (refreshed). Stage 2
> of `plugin-api-stage1` (its D10: built-in → wasm → dlopen/OpenFX). Tests: t3957–t3963, t4151, t4152.

## Why

The owner, `v26.10` lines 101–106: "нужно продумать очень грамотную систему плагинов, которая супер-гибкая … перенести
генераторы в плагины, эффекты … чтобы можно было уже параллельно их разрабатывать … плагины - это про творчество, и
может потребоваться то, что вообще не угадать … плагины могут быть и на разных языках … а кому-то проще на rust+wasm
свой плагин собрать, а кому-то прямо на rust код писать, собирать и запускать(в тот же wasm), те же скрипты/шейдеры
пользовательские будут в будущем." On 2026-11-20 (A11): "все что у нас качается - надо … делать архитектурно в виде
некоторой библиотеки, типо как плагины … их же тоже можно будет качать. вот и сделат библиотеку плагинов."

Stage 1 moved every picked effect, transition and generator into plugin crates against one contract
(`kt-plugin-api`); they are compiled into the program. Nothing from outside can run yet.

## What Changes

- **A wasm host**: a plugin from outside is a WebAssembly module plus its manifest (the same manifest and passport
  as a built-in plugin). The host loads it with the program's wasm runtime (Q1), checks the manifest exactly as it
  checks a built-in one at registration, and registers its kinds in the same registry by family; a project uses it
  by its id as a built-in kind.
- **The same kernels**: an audio kernel's block runs inside the module (f32 48 kHz stereo in, out; its state lives in
  the module's memory, touched only by the render thread); a generator's frame and tone; a transition's numbers; a
  video kernel gives its WGSL passes as text (validated by the same shader compiler, run on the one GPU path); the module only
  gives its numbers (Q2; a pixel reference on the processor was dropped for built-in effects too).
- **A sandbox that is a property, not a promise**: the module imports only what the host gives — no clock, no files,
  no network; a trap (a panic, an out-of-bounds read) is caught, the plugin's kind is switched off with a notice
  naming it, and the project keeps playing with that slot bypassed; a block that runs past its budget is stopped the
  same way (fuel, counted in instructions, not in time). A new package first meets a throwaway process of our own (a trial), so a
  shader that never ends cannot take the window's device. Determinism: the module is pure and its floats are wasm's (IEEE, no fast-math), so the same bytes
  on every machine.
- **A plugin SDK for Rust**: a crate `kt-plugin-sdk` (the passport macros and kernel traits re-exported for
  `wasm32-unknown-unknown`) and a template, so "write in Rust, build, run" is `cargo build --target wasm32` and a
  drop into the library. An installed plugin works from the next start.
- **Plugins are library items**: a plugin is installed, updated and removed through the library of downloads (change
  `download-library`, kind `plugin`), or added from a file ("Add a plugin…"); a project that names a plugin the
  machine lacks keeps its bytes (`Missing`, as today) and the Library offers it.
- **Not here**: native dlopen and the OpenFX bridge (stage 3, change of their own); scene exchange formats (P6) — a
  design of their own (Q3).

## Capabilities

### Modified Capabilities

- `plugins`: the wasm host, the import surface, traps and fuel, the SDK, plugins as library items.
- `ui-reach`: adding a plugin, its notice when switched off, the Library's plugin rows.
- `project-format`: none new (a plugin kind is written by its id as today); the passport of a wasm kind is read from
  its manifest.

## Impact

- New crate `kt-plugin-wasm` (the host; depends on `kt-plugin-api` and the runtime); `kt-plugins-builtin` unchanged;
  kt-app/kt-cli install the wasm host after the built-ins.
- New crate `kt-plugin-sdk` (no host code; builds for wasm32), a template under `src/plugins/template-wasm`.
- kt-gpu: shader text from a plugin goes through the existing naga validation and pipeline cache.
- The binary grows by the runtime (Q1; measured by the executor on the dist build); no libav link changes. wasmtime's signal handlers are off: the crash report's
  handlers stay the only ones.
- Idle: a loaded plugin does nothing until a frame or block needs it.

## Questions to the owner

Decided (2026-11-22, `kkt-26.11.20b` B11, lines 23–24): "wasmtime + WGSL (Recommended)" — Q1 (a) wasmtime, Q2 (a)
video plugins give WGSL; "Отдельным проектом после (Recommended)" — Q3 (a), the layered-scene exchange is a design
of its own after this one.

The questions as asked:

**Q1. The wasm runtime.** Your words: "rust+wasm … собирать и запускать(в тот же wasm)". An audio effect runs a block
every 21 ms on the render thread, so the runtime's speed decides what a plugin can do.
- (a) **wasmtime (compiles the module to machine code once; near native speed; +~10 MB to the program)**
  (*recommended*: audio and generators in real time).
- (b) wasmi (an interpreter; +~1 MB; 10–30× slower — enough for transitions' numbers, not for audio blocks).

**Q2. Shaders from plugins.** A video plugin's speed comes from the GPU.
- (a) **A video plugin gives WGSL text for its passes, checked by the same compiler and run on the one GPU path;
  its CPU code in wasm is the reference for tests** (*recommended*: the same shape as our built-in effects).
- (b) Video plugins run on the CPU in wasm only (simpler, far slower).

**Q3. Exchanging layered scenes with other programs** (your item 6: "передать отрисованную сцену в виде сложной
композиции слоев … и нам можно было бы туда вкладывать-накладывать свои треки-события").
- (a) **A design of its own after this one (OpenEXR multi-layer with depth, USD for 3D scenes, Lottie for vector
  animation are the candidates), next to `real-3d-groups`** (*recommended*).
- (b) In this change.
