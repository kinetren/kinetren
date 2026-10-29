# Design

## Context

See proposal.md — Why. Observed facts (read-only inspection of the tree at 345c94e):

- **The kinds are closed enums in kt-model.** `FxInstance` (`src/libs/kt-model/src/project.rs:5471`,
  serde `tag = "kind"`, `rename_all = "snake_case"`), its exhaustive `tag()` (`:6110`, called "the only
  compile-time lock of the registry") and `all()` (`:6166`, 46 rows: 18 audio incl. `gain` and the
  two-sided `slowed_reverb`, the marker `layer_mix`, 27 video incl. the legacy `cam_motion` and the
  leading `geometry`). `class()` (`:6067`) is one more exhaustive `match`. `TransitionKind`
  (`:79`), `ALL` (`:743`, 33 kinds), `tag()` (`:780`). `GeneratorKind`
  (`src/libs/kt-model/src/genmedia.rs:38`), `all()` (`:591`, 19 menu rows over 15 kinds — the
  test tables are five rows of one kind), `tag()` (`:729`).
- **The passport is already runtime-shaped.** `HasParams` (`project.rs:499`) returns
  `&[ParamSpec]` borrowed from `self` "because for dynamic owners — the public parameter of a
  composition and a plugin — it is built at runtime (R.59)"; `ParamName = Cow<'static, str>`
  (`:6522`), `ParamSpec` (`:6531`); the macros `fx_cast!`/`params_for_struct!`/`params_for!`/
  `flags_for!` (`:6576–6756`) generate direct index access for built-ins.
- **Processing is spread over `match` arms.** `FxInstance::` occurs 174 times in `project.rs`,
  99 in `src/apps/kt-app/src/fx_editor.rs`, 39 in `src/libs/kt-fx/src/audio/nodes.rs`, 38 in
  `src/libs/kt-fx/src/geom.rs`, 25 in `src/apps/kt-app/src/fx_registry.rs`, 20 in
  `src/libs/kt-gpu/src/engine.rs`, 18 in `src/libs/kt-fx/src/video.rs` (plus ~500 in tests).
  `TransitionKind::` appears in 26 files, `GeneratorKind::` in 20.
- **Three video classes decide all code placement** (`src/libs/kt-fx/CLAUDE.md`, "Video effects of
  R.47"): geo (`geom::is_geo`, `geom.rs:47`; primitives `GeomPrim`, `geom.rs:470`: Affine / Clamp /
  Fold / Proj / Warp / Bounds), point-wise (`is_pointwise`, `geom.rs:519`; f64 tap functions
  `tap_grade`/`tap_duo`/`tap_poster`/`tap_vig`, `geom.rs:1783–1823`, each with TWO WGSL mirrors — its
  own pass and the `tap_kind` arm in `src/libs/kt-gpu/src/shaders/geom_fused.wgsl:368–441`), barriers
  (`apply_barrier` in `video.rs`, GPU arms `engine.rs:491–663`). The planner is one function
  (`plan_chain`, `src/libs/kt-fx/src/plan.rs:278`) used by both executors (`apply_video_chain_pooled`,
  `video.rs:191`). The GPU shaders are a fixed `include_str!` table (`engine.rs:772`, 21 passes) plus
  `geom_fused.wgsl` (`engine.rs:921`).
- **Audio state is host-owned already.** `ClipProcessor` (`src/libs/kt-fx/src/audio/mod.rs:367`)
  holds `Vec<(slot, AudioNode)>`; `AudioNode` (`nodes.rs:203`) is an enum with
  build/apply/set_targets/process/tail/reset; `FxStateStore::sync_clip`
  (`src/libs/kt-engine/src/fxstore.rs:135`) diffs chains by clip id and rebuilds on a look-ahead or
  option mismatch. Tail and look-ahead have TWO homes each: `AudioNode::tail()/lookahead()` and the
  static `chain_tail` (`audio/mod.rs:191`) / `chain_lookahead` (`:162`), agreement guarded by t322.
  `MIN_PREROLL_FRAMES` = 4800 (`src/libs/kt-fx/src/lib.rs:20`).
- **Transitions**: the `MODE_*` numbers (`src/libs/kt-engine/src/transitions.rs:18…`) are a contract
  with `composite.wgsl` and are hashed into the GPU key through `put_mix` (`engine.rs:2027`).
- **Injection precedent**: kt-engine knows only `VideoGpuExec` (`src/libs/kt-engine/src/video_plan.rs:248`);
  kt-app/kt-cli build the executor. **Loader precedent** for later stages:
  `src/libs/kt-ffmpeg/src/loader.rs`. **One-home parsing precedent**: `open_pseudo`
  (`src/libs/kt-media/src/factory.rs:40`).
- **Crate graph** (normal deps): kt-fx → kt-model; kt-gpu → kt-model, kt-media, kt-cache, kt-fx,
  kt-engine; kt-engine → kt-model, kt-media, kt-cache, kt-fx; kt-media → kt-model, kt-text; kt-otio →
  kt-model; both apps → everything. So any kt-model edit recompiles every crate but kt-time/kt-text/
  kt-ffmpeg/kt-dirs/kt-version.
- **Sizes**: kt-model 19,252 lines (`project.rs` 9,741, `model.rs` 2,635, `ops.rs` 1,932,
  `genmedia.rs` 1,857); kt-fx 8,502; kt-gpu 4,281 (Rust 2,454 + WGSL 1,827); kt-engine 10,382;
  kt-app 64,108.
- **Build numbers on record** (R.68, `docs/user-feedback.md` "Round R.68", 32 cores, warm tree):
  the cascade from a kt-model edit (`cargo test --workspace --no-run`) 61 s; kt-app + kt-cli 43 s;
  kt-gpu 10 s. They are 2026-09-27 numbers; task 1.1 re-measures before any code moves.
- **The accepted contract**: F-EXT-003 (`docs/features/plugins/hosts/plugin-abi-and-sandbox.md`) and
  `openspec/specs/automation/design.md` ("plugins — built-in → dlopen → OFX, contract: a pure
  function, without clocks, I/O, or state, outside the golden cycle"); the R.59 requirement "a
  plugin remains an interface" (automation spec, t649).

## Goals / Non-Goals

**Goals:**
- One internal interface every built-in effect, transition and generator implements, so the next
  hundred effects are written against it and the wasm host (stage 2) implements the same interface.
- Editing one effect recompiles its family crate and the binaries, never the core.
- Zero visible change: file bytes, references (`GOLDEN_AUDIO`, `GOLDEN_VIDEO`, t328, t322, t223,
  t224, t329), UI and keys stay as they are.

**Non-Goals:**
- Loading foreign code of any kind (wasm, dlopen, OpenFX), a plugin manager window, a plugin
  folder, signing, sandboxing.
- New effects (that is `effects-transitions-catchup`, which is built on this change).
- Time-warping plugins (the rate of slowed&reverb), simulated plugins (physics, F-FX-005), 3D
  meshes, user shaders and scripts (F-EXT-002) — the API reserves the declaration names so they
  are refused loudly, not silently mis-run.
- Moving `Geometry`, `LayerMix`, `SlowedReverb` or the legacy `CamMotion` out of the core.

## Decisions

### D1. A low crate `kt-plugin-api`; the passport moves into it, kt-model re-exports
`src/libs/kt-plugin-api` depends on kt-time and serde only. It receives `ParamSpec`, `ParamKind`,
the param type enum, `ParamName`, `HasParams`, `FlagSpec`, `ParamScope` and the passport macros from
`project.rs:499–6756`, plus new types: `PluginId` (a `&'static str` for built-ins, `Cow` for later),
`ParamValues` (typed values in passport order: number, flag, colour component, choice key, text),
`Manifest`, the four kernel traits, `Registry`, and the minimal kernel I/O types (`ImageRef`,
`ImageMut` — straight RGBA8 with width/height/has_alpha; `VideoCtx` — out size, stage size,
`u`, `burst_index`, `in_scale`; `AudioCtx` — clip position in frames, block length; interleaved
stereo f32 at 48 kHz is the only audio buffer, invariant 1). kt-model `pub use`s the moved names so
no caller changes in stage 1.
Alternatives: keeping the API inside kt-model (every plugin crate would depend on kt-model — the
very cascade this change exists to break); putting it into kt-fx (generators and transitions do not
belong there, and kt-fx depends on kt-model).

### D2. The trait: manifest + kernels, declarations are data
```
trait Plugin: Send + Sync { fn manifest(&self) -> &Manifest; }
Manifest { id, media, passport, variants, ui (category key, keyword key, display name),
           decl: Decl { state, tail, lookahead, threads, alpha, gpu, video_class } }
trait VideoKernel  { fn cpu(&self, p: &ParamValues, src: ImageRef, dst: ImageMut, cx: &VideoCtx);
                     fn tap(&self, p, rgba: &mut [f64; 4], at: (f64, f64)) /* point-wise only */;
                     fn geo(&self, p, cx) -> Vec<GeomPrim>              /* geo only */;
                     fn uniforms(&self, p, cx) -> [f32; 16]; }
trait AudioKernel  { type State; fn build(&self, p) -> State; fn reset(&self, s: &mut State);
                     fn process(&self, s: &mut State, p, buf: &mut [f32], cx: &AudioCtx);
                     fn tail_frames(&self, p) -> i64; fn lookahead_frames(&self, p) -> i64; }
trait TransitionKernel { fn mix(&self, p, progress) -> MixNumbers; fn cpu(..); fn wgsl(&self) -> Option<&'static str>; }
trait GeneratorKernel  { fn frame(&self, values, tick, quality, dst: ImageMut);
                         fn audio(..) /* optional */; fn retune(&self, old, new) -> bool; }
```
(Sketch; names settle in task 2.1.) Declarations are DATA in the manifest, not methods that can
answer differently per call: the registry checks them once (D4). The audio `State` is an associated
type behind a host-side `Box<dyn Any + Send>` slot owned by `ClipProcessor`; the kernel gets
`&mut` only inside `process` — invariant 2 by construction. `tail_frames`/`lookahead_frames` are the
ONE home of each formula: the static `chain_tail`/`chain_lookahead` call them for plugin slots, so
t322's agreement items stop comparing two texts and keep asserting the processor's device.

### D2a. The audio kernel as the node chain uses it (revised 2026-10-29, before stage 3)
The sketch of D2 (`build`/`reset`/`process(p)`) does not carry what the R.47 node chain gives a
node today, and a stage 3 written against it would move `GOLDEN_AUDIO`. Read in `kt-fx`
`audio/nodes.rs` before the move, a node gets: the slot's values cast to the variant's field types
(`apply(fx, structural)`); smoothing targets along its curves once per chunk, ONLY when the clip
has automation (`set_targets(auto, slot, s)` — the gate keeps the bytes of old chains); its LFO
frequency curve copied at bind time and integrated per frame (`lfo_phase` → `param_integral`); for
pitch, curves read by grain position with segment shapes; the sidechain key of the same window
(compressor); and a project setting (the pitch engine) that rebuilds the processor. So:
```
trait AudioKernel  { fn build(&self, opts: &HostOpts) -> Box<dyn AudioState>; }
trait AudioState: Send {
    fn apply(&mut self, k: &[f64], structural: bool);      // the row-cast values (Store), all of them
    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64); // copies what process needs (no model access later)
    fn set_targets(&mut self, c: &dyn SlotCurves, s: f64); // the host calls it only with automation
    fn process(&mut self, buf: &mut [f32], pos: i64, key: Option<&[f32]>);
    fn lookahead(&self) -> i64; fn tail(&self) -> Tick; fn reset(&mut self); }
trait SlotCurves { fn value(&self, param, base, s) -> f64;   // kt-model's param_at, shapes included
                   fn points(&self, param) -> &ParamPoints;
                   fn owned(&self, param) -> Box<dyn CurveRead>; } // a by-position reader (pitch)
```
`HostOpts` carries the project settings a kernel declares it reads, by name (today only
`pitch_engine`); a change of one is structural, as `ChainOpts` is now. The state is a
`Box<dyn AudioState>` held in `ClipProcessor`'s node list next to the core nodes (`layer_mix`,
`slowed_reverb`), so the dynamic call is per chunk, and the per-frame loop is inside the plugin.
What moves DOWN into `kt-plugin-api` (pure, no model): `ParamPoints` and `param_integral` (the LFO
integrates per frame and must not call back into the host), and the DSP primitives the nodes share
(`SmoothedParam`, `Biquad`, `DelayLine`, `EnvFollower`, `lfo_phase`, `softclip`, `apply_width`, and
the comb/allpass of the core reverb) as `kt_plugin_api::dsp`; kt-model and kt-fx re-export them, so
each formula keeps one home. `param_at` and segment shapes stay in kt-model behind `SlotCurves`.
Byte rule: `apply` gets each row cast through its `Store` (an `f32` field reaches the node as the
same `f32`); `chain_tail`/`chain_lookahead` ask the plugin's `tail_frames`/`lookahead_frames` for a
plugin slot and t322 keeps comparing them with the built state.

### D3. Ids are today's tags; a missing id is a kept slot
`PluginId` = the string `tag()` returns. `FxInstance` gains one variant `Plugin(PluginSlot)`
(`id`, `ParamValues`, plus nothing else — curves stay in `fx_auto` on the clip as today) and one
variant `Missing { id, raw: serde_json::Value }`. Serde for the enum becomes a hand-written
`Serialize`/`Deserialize` that, for a plugin slot, writes `kind` and then the passport rows in
passport order, omitting rows the passport marks `omit_default` (the `skip_serializing_if` of today,
e.g. `Gain.pan`); reading an unknown `kind` asks the registry, and with no answer keeps the raw
value. The frozen pre-move serializer (a verbatim copy of today's derive output inside t973, the
t643 "frozen oracle" technique) proves byte identity for every moved kind; this is also why a
moved kind's passport MUST list its rows in the old field order — a reordering is caught by t973,
not by a reviewer. No `kt.v` bump: nothing new is written for built-ins, and `Missing` writes back
what it read.
Alternatives: a numeric plugin index in the file (moves when a plugin is inserted); a new
`plugin: {id, params}` wrapper object (changes every file's text — R.70 made the format git-friendly
precisely so that unchanged content gives unchanged lines); failing to open a file with an unknown
id (makes every shared project hostage to the slowest machine — the reason `effects_foreign`
exists on the OTIO door, `src/libs/kt-otio/src/convert.rs:48`).

### D4. Revision: refusal at registration is split by media — video is pure, audio declares
F-EXT-003 says "a plugin that declares state between frames or a tail is refused AT LOAD". Revision:
**the rule holds for video kernels and is narrowed for audio** — reason: the rule's own argument is
post-FX memoization ("letting such a plugin in means starting to lie with the cache"), which exists
for frames only; audio has no post-FX cache (invariant 2: prefetch fills only the pre-FX cache), and
every built-in reverb, delay, limiter and pitch node already keeps state the host owns and diffs by
clip id (invariant 3), with a declared tail and look-ahead (PDC, R.48). Refusing them would refuse
the very effects stage 2 moves. What stays refused: any video state/tail/look-ahead; any kernel that
is not callable concurrently (`threads: Single` — the thumbnail service, the preview and the export
call video kernels from several threads; OFX's `kOfxImageEffectRenderUnsafe` is the cautionary
example in F-EXT-003); an audio look-ahead whose maximum over the passport range reaches
`MIN_PREROLL_FRAMES` (the seek warm-up would start from an empty ring); a built-in point-wise or
barrier kernel without WGSL (built-ins keep the GPU default); a geometry kernel that offers WGSL.
The checks run in `Registry::register`, before any frame — t972.

### D5. The GPU side is assembled from the registry; the fused pass stays closed for geometry
`GpuFxEngine::new(ctx, &Registry)` (today `new(ctx)`, `engine.rs:801`) builds:
- **point-wise**: each plugin's `tap_<id>` WGSL function is pasted into `geom_fused.wgsl`'s tap
  dispatch (today the hand-written `if tap_kind == N` chain, `:368–441`) and into a generated
  one-function pass for the barrier position — the "two WGSL mirrors" of the kt-fx checklist become
  one text used twice;
- **barrier**: the plugin's fragment pass is compiled over `common.wgsl` with a fixed uniform
  block of 16 floats (`uniforms()` computed on the CPU in f64 → f32 once per frame, as the warp
  precompute does today);
- **geometry**: no WGSL; `geo()` returns primitives from the closed set, so the Jacobian
  supersampling, the two primitive loops (`geo_map` and `geo_sample`) and f32 parity stay in one
  hand-checked file. A new primitive is a core change (rare: R.47 added one, Warp with modes, for
  five kinds).
The dispatch numbers are assigned in the assembler's id order at engine creation; the assembled
text is therefore a pure function of the registry (invariant 11, t974). Every key that is HASHED
(`put_mix`, the GPU plan hash, the post-FX key) uses the id string, never the dispatch number —
otherwise inserting a plugin would shift keys silently.
`plan_chain` asks `manifest.decl.video_class` where it asks `is_geo`/`is_pointwise` today; both
executors keep calling the same planner — invariant 7 is untouched by construction.
Alternatives: arbitrary WGSL in the fused geometry map (breaks the f32-parity argument of
"normalisation is in the affines" and the K× rule); a separate pass per plugin always (loses the
fusion that R.44 bought — "a series of any length is one pass"); naga-level shader linking (a
second shader toolchain to keep in sync with wgpu 30).

### D6. Family crates and one assembler; the registry is injected
```
src/plugins/fx-video-color      color_grade duotone posterize vignette
src/plugins/fx-video-stylize    sharpen pixelate halftone noise vhs edges glow
src/plugins/fx-video-distort    lens_distort twirl ripple kaleidoscope cylinder3d corner_pin mirror event_grid pan3d
src/plugins/fx-video-blur       blur radial_blur drop_shadow chromatic_aberration greenscreen
src/plugins/fx-audio-dynamics   gain compressor gate limiter
src/plugins/fx-audio-tone       filter eq distortion bitcrusher stereo_width
src/plugins/fx-audio-modulation tremolo auto_pan vibrato chorus flanger phaser delay pitch_shift
src/plugins/transitions-basic   crossfade wipe iris clock barn luma push slide dip flash …
src/plugins/transitions-styled  whip blur_dissolve pixelate glitch film_burn light_leaks zoom_blur shake …
src/plugins/transitions-volume  flip cube door page_peel page_roll swap
src/plugins/generators-basic    solid gradient corners checker noise transparent rect ellipse line
src/plugins/generators-test     smpte timecode leader table window
src/plugins/generators-title    text            (depends on kt-text — the declared exception)
src/libs/kt-plugins-builtin     BUILTIN_IDS (closed, ordered) + fn builtin() -> Registry
```
The grouping follows the existing picker categories so a family is what one person edits in
parallel; the final split is decided in each stage by line counts. kt-app and kt-cli call
`kt_plugins_builtin::builtin()` once in `run`/`main` and hand the `Arc<Registry>` to the model's
deserializer (a process-wide install-once slot in kt-plugin-api: the format reader cannot take an
argument through serde), to kt-fx/kt-engine (`plan_chain`, `ClipProcessor`), to kt-gpu (D5) and to
kt-media's factory. Tests install it through the same function; a second install with a different
id list panics in debug. The core crates gain only `kt-plugin-api` as a dependency and dev-depend on
the assembler. t975 reads the manifests and guards the graph.
Alternatives: one crate per effect (≈100 crates: cargo's per-crate overhead and 100 `Cargo.toml`s
for no measured gain — revisit with numbers in stage 6); `inventory`/linker-section registration
(order would depend on the linker; the closed ordered list is what the goldens need); a feature
flag per family (features unify across the workspace and would rebuild the core on toggle).

### D7. The UI reads the registry; custom widgets stay in kt-app keyed by id
`fx_registry.rs` builds `FxEntry` from manifests: `name` = the manifest's display name (an industry
term, not translated, as today — t48), `category`/`keywords`/variant names = catalog key PATHS that
the manifest names as strings and `i18n` resolves (kt-app's `build.rs` already compiles catalogs into
tables; a guard checks that every key a built-in names exists in all five catalogs — t976).
Parameter forms stay `param_row`/`params_grid` over the passport (rule "a numeric field is drawn ONLY
by `param_row`", automation design). Kind-specific widgets (the EQ graph, colour rows, the camera
handles) stay in kt-app and are looked up by plugin id; a plugin cannot ship UI in this change.
Labels of a dynamic owner already have their path (`ParamSpec::label`, R.59, t684) — that is what an
external plugin will use in stage 2.

### D8. The closed built-in list is the only input of the references
`BUILTIN_IDS` in kt-plugins-builtin is a `const` slice in a fixed order. t328 (`GOLDEN_ALL`), t329,
t322, t223/t224 and the generator goldens iterate it (plus the core-owned kinds). The test door
(`Registry::register_for_test`) exists so t976 can prove the separation. This keeps the R.47
completeness lock: adding a built-in id without a golden row fails the lock.

### D9. Stages and the pilot
1. API, registry, assembler, injection, and ONE pilot: **Posterize** — the smallest kernel that
   crosses all four executor positions (CPU barrier, CPU tap of the fused pass, its own GPU pass, the
   GPU tap arm), so D5 is proven before a single family moves.
2. Audio FX (17 plugin kinds; the warp stays core).
3. Video FX (25 kinds; `geometry` and `cam_motion` stay core).
4. Transitions (33).
5. Generators (15 kinds, the title last — if `text_anim_curves`/`migrate_gen_to_events`
   (`genmedia.rs`) must stay in kt-model, the title stays a core kind and the reason is recorded).
6. Measured acceptance (t977) and the cleanup: the enum variants, `tag()` arms and hand-written
   checklists of moved kinds are deleted.
Each stage ends green and committed; t973 grows the list of moved kinds every stage.

### D10. Revision: the order of external hosts — wasm before dlopen/OpenFX
F-EXT-003 and the automation design fix "built-in → dlopen → OFX". Revision: **built-in (this
change) → wasm host (stage 2) → dlopen and the OpenFX bridge (stage 3)** — reason: the owner now
names Rust→wasm as the likely external route (ktprompts P5: "someone finds it easier to build a
rust+wasm plugin… compile and run it (in the same wasm)"), and wasm turns three lines of the
contract from promises into properties: no clock and no I/O (a module has only the imports we give
it), a caught trap instead of a segfault (F-EXT-003 "Cons": "a panic can be caught, a segfault
cannot"), one binary for Linux and Windows. dlopen keeps its value for OpenFX, which is C ABI by
definition, and moves behind wasm. The interface of D2 is what the wasm host will implement; this
change writes no wasm code.

## Risks / Trade-offs

- [The hand-written serde of D3 drifts from today's derive output for a corner case (a `u8`
  written as `5.0`, a defaulted field reappearing)] → t973's frozen oracle covers every moved kind
  with default AND non-default values; a stage cannot be committed red.
- [Dynamic dispatch on hot paths (per-pixel tap, per-block audio) costs speed] → measured in
  2.6: a dynamic `tap` per pixel made Posterize 63 % slower alone, so the video kernel got
  `cook` (parameters once per plan) and `frame` (the barrier position as one dynamic call, its
  provided body compiled per kernel; the loop text is `frame_with`); inside the fused pass the
  per-sample `tap` stays dynamic and costs nothing measurable (−0.9 %). Taps are called
  per pixel only on the CPU reference path; the trait call is per frame or per block, the inner
  loop stays monomorphic inside the plugin; measured in stage 1 on release with `kt-cli probe --time`
  and the t322 timing, against the pre-move build. A slowdown over 3 % on the probe blocks the stage.
- [The assembled WGSL hits an FXC limitation on D3D12 (the windows-build lesson: FXC rejects a write
  through a dynamic vector index)] → the generated dispatch uses the same `if` chain form that passes
  today; the Windows CI job runs t974.
- [Install-once global registry hides a missing install in some test] → the deserializer without a
  registry fails loudly in debug ("no registry installed"), not silently `Missing`.
- [The build win is smaller than hoped because kt-app itself takes 40 s to link and compile] → t977
  reports the numbers honestly; the acceptance is the recompile SET (no core crate) plus the time
  next to the R.68 and stage-1 baselines; if the time does not improve, the result is recorded, not
  hidden, and the next lever (splitting kt-app) is named in the report.
- [Two places still change together for a new kind — the plugin crate and its catalog keys in
  kt-app] → accepted: translations live with the program (five catalogs, font subsets); t976 catches a
  missing key.

## Migration Plan

No user-visible migration: file text, references and UI are byte-stable (t973, t328, t322, t223).
Rollback per stage is a revert of that stage's commit — each stage keeps both paths compiling until
its kinds are deleted in stage 6.

## Open Questions

1. **Which wasm runtime for stage 2?** Recommendation: `wasmtime` with NaN canonicalisation and fuel
   limits for determinism and runaway protection; `wasmi` (a pure-Rust interpreter) as the fallback if
   the binary size or the Windows cross-build objects. Does not change this change's API.
2. **May EXTERNAL audio plugins (stage 2) declare state and a tail like built-ins?** Recommendation:
   yes, with the same host-owned state and the same refusals (look-ahead < 4800 frames); a plugin that
   lies about its tail is cut at the declared length — the honest-tail rule of the audio spec.
3. **Should the rate of slowed&reverb become a "time plugin" kind later?** Recommendation: not before
   F-SPD-002 (retime curve) is decided — the warp has two axes, a link-group sync and the prefetch
   window in the model (`sync_warp_to_link_group`, `warp.rs`); a plugin form would have to carry all
   three. The manifest reserves `decl.time = Warp` so a plugin asking for it is refused by name.
4. **Family granularity** (D6): recommendation — the 13 families above; revisit only with t977
   numbers.
