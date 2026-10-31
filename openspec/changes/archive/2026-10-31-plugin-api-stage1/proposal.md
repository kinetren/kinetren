# Proposal

## Why

Every effect, transition and generator is a variant of a closed enum in kt-model
(`FxInstance::all()` — 46 kinds, `TransitionKind::ALL` — 33, `GeneratorKind` — 15 kinds in 19
menu rows), processed by `match` arms in kt-fx, kt-gpu, kt-engine, kt-media and kt-app. Adding
one effect touches four crates, and the kt-model edit recompiles the whole workspace (R.68
measured the cascade at 61 s for `--no-run`; kt-model is 19.3k lines, `project.rs` alone 9.7k).
The owner (2026-10-27, item 7): "even now the build takes very long, I would not like to lose
speed, and I want to unify the program and the interfaces at last — so plugins come before the
effects", and chose the first stage: **an internal API plus crates** — the built-ins move into
their own crates behind one interface; wasm and dlopen come later. The ktprompts section P1–P6
sets the horizon: a super-flexible system, generators and effects developed in parallel, plugins
for creative work (the rate of slowed&reverb, physics for letters, 3D), several languages (Rust
built-in, Rust→wasm, user scripts and shaders).

## What Changes

- New crate `src/libs/kt-plugin-api` low in the graph (depends on kt-time and serde only): the
  parameter passport (`ParamSpec`, `ParamKind`, `ParamName`, `HasParams`, `FlagSpec`) moves here
  from kt-model (kt-model re-exports it, so no caller changes); the plugin manifest with its
  declarations (media, video class, state, tail, look-ahead, threads, alpha rule, GPU kernel);
  the kernel traits for video, audio, transition and generator plugins; the registry.
- **A registry replaces the closed enums step by step.** A plugin id IS the current `tag()`
  string, so `.kt` files keep their text byte for byte. Core-owned kinds stay enum variants:
  `LayerMix`, `Geometry`, `SlowedReverb` (the warp), `CamMotion` (read-only legacy).
- Family crates under a new workspace directory `src/plugins/` (e.g. `fx-video-color`,
  `fx-audio-dynamics`, `transitions-wipes`, `generators-basic`), each depending only on
  kt-plugin-api; one assembler crate `kt-plugins-builtin` lists the closed built-in set and is
  depended on only by kt-app, kt-cli and test targets. The registry is injected at run time
  (the `VideoGpuExec` pattern), so editing one effect recompiles its family crate, the
  assembler and the two binaries — not kt-model, kt-fx, kt-engine, kt-gpu.
- kt-gpu assembles the fused geometry pass and the per-kind passes from the registry's WGSL
  pieces at engine creation; the chain plan stays in one place (`plan_chain`), so invariant 7's
  one composition path holds; a geometry plugin expresses its map only in the closed primitive
  set and never injects WGSL into the fused map.
- kt-app's FX picker, transition picker, generator menu and parameter forms read the registry
  (display name, category, keywords and variants come from the manifest; labels stay catalog
  keys in all five languages).
- The golden tests (`GOLDEN_ALL` t328, the audio harness t322, the transition registry t223)
  iterate the CLOSED built-in id list of `kt-plugins-builtin`; nothing registered elsewhere
  enters them.
- **Revision of F-EXT-003's order:** built-in → **wasm host (stage 2)** → dlopen / OpenFX
  (stage 3); and of its refusal rule: a declared state, tail or look-ahead is refused for VIDEO
  kernels (post-FX memoization), admitted for AUDIO nodes whose state the host owns on the render
  thread.
- Migration in stages: API + registry + one pilot (Posterize) → audio FX → video FX →
  transitions → generators → a measured build-time acceptance against the R.68 numbers.
- Deferred, explicitly: the wasm host, dlopen, OpenFX, scripting (F-EXT-002), user shaders,
  time-warping plugins (the rate of slowed&reverb), simulated (physics) plugins, plugin-owned UI.

## Capabilities

### New Capabilities

- `plugins`: the plugin API and registry — the manifest and its declarations, refusal at
  registration, ids equal to file tags, a missing plugin in a file, the crate graph and the
  rebuild scope, the closed built-in set versus the golden tests.

### Modified Capabilities

- `automation`: the "plugin remains an interface" requirement becomes a working API; the
  refusal rule is split by media (video refused, audio admitted with host-owned state).
- `video-fx`: the video registry closed by the golden cycle is the built-in plugin set; the
  kind's class and GPU kernel come from its manifest; the fused pass is assembled from the
  registry.
- `audio-fx`: the audio registry and the parameterized harness run over the built-in plugin set;
  tail and look-ahead are manifest declarations in one home.
- `transitions`: the vocabulary is the built-in transition plugin set (still exactly 33); the
  CPU reference and the WGSL mirror of a kind live in one plugin crate.
- `generators`: generator kinds come from the registry; the pseudo-path and the event carry the
  plugin id; the frame stays a pure function of (values, tick, quality).

## Impact

- New crates: `src/libs/kt-plugin-api`, `src/libs/kt-plugins-builtin`, family crates under
  `src/plugins/` (workspace members in `src/Cargo.toml`).
- Code: kt-model (`project.rs` passport macros and `FxInstance`/`TransitionKind`, `genmedia.rs`),
  kt-fx (`plan.rs`, `video.rs`, `geom.rs`, `audio/nodes.rs`, `audio/mod.rs`), kt-gpu
  (`engine.rs`, `shaders/geom_fused.wgsl`, `shaders/composite.wgsl`), kt-engine
  (`transitions.rs`, `video_plan.rs`, `fxstore.rs`), kt-media (`generator.rs`, `factory.rs`),
  kt-app (`fx_registry.rs`, `fx_editor.rs`, `gen_ui.rs`, `transition_editor.rs`, `i18n/*.toml`),
  kt-cli (registry injection).
- No project-format version change: ids are today's tags; a file naming an id that this build
  does not have keeps its bytes (a "missing plugin" slot) instead of failing to load.
- Docs: `docs/features/plugins/hosts/plugin-abi-and-sandbox.md` (F-EXT-003) gains a dated
  revision section; crate `CLAUDE.md` checklists "add an audio/video effect" are rewritten in
  the stage that moves each family.
- Tests reserved: t972–t977.
