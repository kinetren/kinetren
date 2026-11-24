# Design: wasm-plugin-host

> IMPLEMENTATION-READY (spec-engine, revision 2, 2026-11-25, orch `fc60a5d9`). The owner answered Q1 (a) wasmtime, Q2 (a) WGSL, Q3 (a) later (kkt-26.11.20b B11). Code is named by function and file.

## Context (what exists, and what a plugin from outside meets)

- **The contract**: `kt-plugin-api`. `Manifest { id, media, passport: Vec<ParamSpec>, rows: Vec<RowDecl>, decl: Decl, ui }`; kernels `VideoKernel`, `BarrierKernel`, `AudioKernel → AudioState`, `TransKernel`, `GenKernel`. `Registry::register` refuses a bad declaration before any frame.
- **What it takes from compiled-in code**: `Decl.gpu: Gpu::Wgsl(&'static str)`, `fn` pointers for tail and look-ahead (`FramesFn`), `&'static str` in `UiDecl`, `GenDecl`, `foreign_rows`; `BarrierKernel::gpu` builds its graph in code.
- **The registry is one `OnceLock` of the process** (`kt_plugin_api::install`, about fifty readers through `installed()` / `plugin_slot::registry()`); kt-gpu assembles its shader set and its slots from it when an engine is made; `StreamStage.plugin` and others keep `&'static` plugins. So the list of plugins is fixed when the program starts.
- **A kind this build lacks** reads as `Missing` and keeps its bytes. `Made::salt` (the disk tier) and `picture_key` name a plugin by its source hash (`set_source`) and WGSL.
- **kt-crash** owns SIGSEGV, SIGBUS, SIGILL, SIGFPE: no runtime may put a handler over it.
- **The libva driver's trial** (`hw_trial`): our own binary started again meets what may fault. The same pattern serves here (D6).

## D1. The package

`plugin.toml`, `plugin.wasm`, `*.wgsl`, at most 32 MB, in a zip `.ktplugin` or a folder `<library>/plugin/<id>/<version>/`. One package holds up to 32 kinds. `plugin.toml`:

```toml
abi = 1
[package]   # id (reverse domain), version (semver), name, author, licence
[[kind]]    # id (lower snake_case; unique in its family), media = "audio_fx" | "video_fx" | "transition" | "generator"
            # group, label, icon
tail_ms = 0            # audio: a constant, the kernel's worst
lookahead_frames = 0   # audio: a constant; the registry's bound applies
class = "pointwise" | "barrier"          # video
alpha = "preserves" | "produces"         # as AlphaRel
[[kind.row]]           # name, kind = "smooth" | "choice" | "flag" | "color", min, max, default, step, unit, label, keys
[[kind.pass]]          # video barrier: wgsl, inputs = ["source" | "prev" | "saved:n"], save = n, scale = 1 | 2 | 4
```
- `kt-plugin-wasm::package` parses it into a `Manifest` and the SAME `Registry::register` judges it. Strings and WGSL become `&'static` by `Box::leak`, once per (package hash) and process: bounded by what is installed. `FramesFn` is replaced in `Decl` by `Frames { Of(FramesFn), Fixed(i64) }` (a change of two fields' type; every built-in keeps its `fn`).
- Labels come from the package (`ParamSpec.label` non-empty is the existing form for a dynamic owner); no catalog key.
- Ids: a kind id that is a built-in's is refused ("the id is the program's own"); two installed packages with one id: the first by package id wins, the other is refused with its name. `.kt` writes the kind id as for any plugin.
- **The version in keys**: `set_source(family, id, xxh3(package bytes))`.

## D2. The module's imports (all it can call)

`kt.log(level, ptr, len)` (the journal, at most 20 lines a second a package), `kt.math_{sin,cos,tan,exp,pow,log10,atan2}{,f}` (the frozen math: a plugin's sine is the program's). Nothing else: no clock, no random (a seed is a row), no files, no network, no threads, no WASI. A module importing anything else is refused at registration naming the import.

**wasmtime 48, `default-features = false`, features `runtime`, `cranelift`, `std`, `parallel-compilation`**: no WASI, no cache, no async, no component model. Configuration, each for a reason:
- `signals_based_traps(false)`: traps are explicit checks; kt-crash's handlers stay the only ones;
- `consume_fuel(true)`: a budget in instructions, the same on every machine; epochs are a clock;
- `cranelift_nan_canonicalization(true)`, `wasm_relaxed_simd(false)`, `wasm_threads(false)`: one result everywhere (invariant 11); fixed SIMD is allowed;
- `StoreLimits`: 64 MB of memory, 1 table, 1 memory an instance;
- the call boundary clears and restores MXCSR / FPCR (no flush-to-zero set by another library leaks in).

A module is compiled from its wasm at the program's start by a background lane, kinds the opened project uses first; a build of a project waits for them (as it waits for a missing source's placeholder: the slot reads `Missing` until then and the next build takes it). Nothing compiled is read back from disk (`Module::deserialize` trusts its input).

## D3. Kernels across the boundary (ABI 1)

Exports: `kt_abi() -> i32`, `kt_alloc(n) -> i32`, and per family, kinds by their index in the manifest. Every buffer lies in the module's memory, allocated once at build by `kt_alloc`.

- **Audio** (`AudioState` over one `Store` + instance per slot state; created at `build`, off the render thread where the builder builds it, kept by `FxStateStore` as any): `a_new(kind) -> h`, `a_apply(h, rows, n, structural)`, `a_targets(h, rows, n)` (the HOST evaluates the slot's curves at the chunk's progress and hands plain numbers: no curve code in a module),
  `a_place(h, start_lo, start_hi, fps_num, fps_den)`, `a_process(h, buf, frames, pos_lo, pos_hi, key | 0)` in place, `a_reset(h)`. `fork` is `None` (the echo of its chain is cut at the event's end, the trait's documented answer). One call a chunk, no host allocation (t2684's counter runs over a wasm slot).
- **Transition**: `t_pack(kind, rows) -> 8 × f32` at `TransMix::of`, memoized by the rows; its WGSL is `trans_<id>` of the package.
- **Generator**: `g_frame(kind, rows, pts_lo, pts_hi, w, h) -> ptr` on the media worker (its own instance per source: invariant 5 reads as for any source); `g_tone`, `g_gate`.
- **Video**: the GPU runs the package's WGSL: `tap_<id>` (pointwise, pure as `impure_tap` demands) or the passes of `[[kind.pass]]` (barrier: a fixed graph through `check_graph`). The numbers: `v_cook(kind, rows, n, t_lo, t_hi, u, w, h) -> 8 × f64` once a frame on the plan's thread. There is no pixel code in wasm: the byte reference was dropped for built-ins too
  (`FrameTol` against stored frames is the plugin author's test, `kt-cli plugin test`). Q2's "CPU reference for tests" is answered by that.
  `reads_time`/`reads_progress` (cached-final-render) stay the default `true`.

## D4. Faults

Each call has fuel: audio 20 000 an input frame (a block of 1024: 20 M, about 5 ms on the slowest machine measured: the executor states the number), `v_cook`/`t_pack` 10 M, `g_frame` 400 a pixel. A trap or an empty tank: the call gives silence / the identity / a transparent frame, the KIND is faulted for the session (an atomic), its slots are bypassed at the next build, one notice names the package, the kind and the reason
("stopped: it worked longer than 5 ms for a 21 ms block"), the journal has the module's backtrace. The project's bytes do not change; the next start tries again. An export with a faulted kind refuses to start and names it; a fault during an export ends it named (a silent bypass would be a wrong file).
A shader that does not compile: the kind is faulted the same way (`gpu-errors-survive`'s scope per pipeline).

## D5. When it takes effect

Added, updated, removed: at the next start ("Installed. It works after Kinetren is started again" with a "Restart now" that saves through the close's own path). Why not at once: the registry, the engines' shader sets and slots are made once; an append-only second registry would reach fifty readers for a moment a person meets once a plugin. VEGAS, Premiere and Resolve ask the same.
A project already naming the kind shows `Missing` until then.

## D6. A trial at installing

`kinetren --plugin-trial <dir>` (served at the top of `main`, beside `hw_trial::serve`, before any file of the person): registers the package, builds and runs every kernel at the defaults, the bottom and the top of its rows over a second of audio, three frames, and draws every pass at 64×64 and 1920×1080 on an offline device; 30 s; it prints its verdict. A death, a hang (a
shader that loops for ever ends the device, not our window) or a refusal → the package is not installed and the reason is shown. Its result is kept with the package hash and the GPU driver's version; another driver asks again at the start, in the background.

## D7. The SDK

`libs/kt-plugin-sdk` (builds for `wasm32-unknown-unknown`, no host code): `#[derive]`-free macros `audio_kernel!`, `generator!`, `transition!`, `video_cook!` over safe traits (`Audio`, `Generator`, …) and `kt::math`. `src/plugins/template-wasm`: a gain, a solid colour, an invert (pointwise), a two-pass blur. Not a workspace member of the default build; CI builds it for
wasm32 and t3957 loads it; a prebuilt copy for a machine without the target is made by the test's own `wat` text (the sandbox tests are `wat` files: no toolchain needed).
`kt-cli plugin {check|test|pack} <dir>`: the parser's and the registry's verdict; the trial; the zip.

## D8. Installing

`download-library` (its section 5.2) lists kind `plugin` from the catalogue; until it is in, Preferences › "Add a plugin…" takes a file: unpack by the library's own rules (`kt-library::net`'s unpack: no path outside, no links, sizes), hash, trial, place. A removed plugin leaves `Missing`.

## D9. Class walk

| Door | Here |
|---|---|
| every reader of the registry | sees one list, as today |
| `Decl` `fn` pointers, `&'static str` | `Frames`, leaked strings (D1) |
| the picker, the node editor, the passport's grid, curves, links, presets, published rows | the same rows: nothing of theirs asks where a plugin came from; a "plugin" mark on the card and the window's header |
| icons, categories, i18n | the package's label; category from `group`, "Plugins" when unknown |
| pair devices (`Decl.pair`), stream kinds, geometry warps, time warps, sidechain roles, `reads_across_edges` | not in ABI 1: refused in the manifest with the reason |
| simulation solvers (`sim` family) | a later ABI |
| the audio kept mix, scene hash | the kind's values are in the model text; `AUDIO_PATH_VERSION` + the package hash salt it on disk |
| export determinism | pure modules, fuel in instructions, canonical NaN; the WGSL as any effect's |
| kt-cli | loads the installed packages as the app does; `--no-plugins` |
| Windows, ARM, Flatpak | cranelift has both; JIT memory needs `allow-jit`-like permission nowhere we ship (Flatpak allows it); the PE imports rule (no new DLL) is checked by dist-guard |
| size | wasmtime's runtime and cranelift: measured by the executor on the dist build, written here (the owner was told about 10 MB) |
| formulas, user shaders | unchanged |
| OpenFX | `openfx-host`: native code never in this process |

## D10. Idle

A loaded module is memory; nothing runs until a frame or a block asks. The trial and compilation run once, in the background, and end.

## Rejected

- **dlopen first** — a native plugin's crash is the program's crash.
- **A scripting language of our own** — "no invented language".
- **Compiled code cached on disk** — a file that is executed; compiling takes a second once a start.
- **Pixels in wasm** — a copy off the GPU and back per effect; the owner chose WGSL.
- **wasmtime's signal handlers, epochs, WASI** — D2.
