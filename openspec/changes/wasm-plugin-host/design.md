# Design: wasm-plugin-host

> DESIGN ONLY, revision 1 (2026-11-20), base `b18f97e4`. Q1–Q3 in `proposal.md`; defaults recommended.

## Context

- `kt-plugin-api`: manifest + passport (rows, units, limits), kernels by family (`AudioKernel::build →
  AudioState`, `VideoKernel` with `cook`/`tap`/`frame` and a GPU pass graph, `TransKernel::pack`, `GenKernel::frame/
  tone/gate`), the registry by family; a kind this build lacks reads as `Missing` and keeps its bytes (spec
  `plugins`). The host refuses an incompatible declaration at registration.
- `plugin-api-stage1` D10: built-in → wasm host → dlopen/OpenFX.
- `download-library` (design): one catalogue, one folder `<local>/library/<kind>/<id>/<version>/`, `library::resolve`,
  kind `plugin` reserved.
- Invariants 1, 2 (audio f32 48k, FX state on the render thread), 7 (one GPU path), 11 (determinism).

## D1. The module and its manifest

A plugin package = `plugin.toml` (the manifest: id, family, version, the passport, declared state/tail/look-ahead,
for video the pass graph and its WGSL files) + `plugin.wasm` + WGSL files. The host reads the manifest with the SAME
parser and check as a built-in's declaration (one door), then instantiates the module once per process (compiled
module cached under the library folder by the module's hash).

## D2. The import surface (all a module can call)

`kt.log(level, str)` (into the journal, rate-limited), `kt.math.*` (the frozen math of `kt_plugin_api::math`, so a
plugin computes sin/exp to the program's bits), memory growth up to a limit (64 MB per instance). No clock, no
random (a seed comes as a parameter), no files, no network, no threads. A module importing anything else is refused
at registration, naming the import.

## D3. Kernels across the boundary

- Audio: `build(opts) -> handle`; `process(handle, in_ptr, out_ptr, frames, params_ptr)`; `seek(handle)`;
  `drop(handle)`. Buffers live in the module's memory, written by the host before the call; the render thread is the
  only caller (invariant 2). No allocation by the host per block (the buffers are allocated at `build`).
- Generator: `frame(params, pts, w, h) -> rgba` into module memory; `tone`, `gate` as numbers.
- Transition: `pack(params) -> [f32; 8]`.
- Video: the WGSL pass graph from the manifest runs on the GPU path exactly as a built-in barrier's; `tap`/`frame` in
  wasm are the CPU reference used by the conformance tests and never in the preview.

## D4. Traps, fuel, switching off

Each call runs with fuel (audio: a budget per block proportional to frames, measured on the reference machine ×10;
generator frame: by pixels). A trap or exhausted fuel: the call returns silence / a transparent frame / the identity
numbers; the kind is marked faulted for the session; every slot of that kind is bypassed in the next build; a notice
names the plugin and the reason once; the journal records it. The project's bytes are unchanged; the next start
tries again.

## D5. The SDK

`kt-plugin-sdk` re-exports the passport macros and kernel traits of `kt-plugin-api` for wasm32 and provides
`export_plugin!(MyKernel)` generating the D3 exports. `src/plugins/template-wasm` is a gain + a solid-colour
generator, built in CI for wasm32 and loaded by a test.

## D6. Installing

From the Library (kind `plugin`: catalogue entries with sha256) or "Add a plugin…" (a `.ktplugin` zip of D1's
package): unpacked into the library folder, registered at once (a project already using its id rebuilds and plays it).
Removing a plugin a project uses keeps the project's bytes (`Missing`).

## D7. Class walk — "foreign code that can break the program"

| Door | Today | Here |
|---|---|---|
| built-in plugins | compiled in, trusted | unchanged |
| wasm plugins | none | sandboxed (D2), trapped (D4) |
| FFmpeg (system library) | dlopen, a driver may fault | the separate process for libva (export) — unchanged |
| user shaders (WGSL) | none | validated by naga; a GPU error is a journal line (gpu-errors-survive) |
| formulas in curves | compiled closures, ours | unchanged |
| dlopen / OpenFX | none | stage 3 (a process boundary, F-EXT-001) |

## D8. Idle

A loaded module costs memory only; nothing runs unless a frame or a block asks.

## Rejected

- **dlopen first** — a native plugin's crash is the program's crash; wasm makes the sandbox a property (stage 1 D10).
- **A scripting language of our own** — the owner: "no invented language" (acceleration scope); Rust→wasm is the route he
  named.
