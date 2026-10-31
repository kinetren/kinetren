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
  its presets and R.53–R.58 migration are kt-model's. The barrier video kinds move in
  `plugin-api-barriers`.
- **The order of external hosts: wasm before dlopen/OpenFX (D10).** It revises F-EXT-003's
  "built-in → dlopen → OFX". A wasm module has only the imports we give it, so "no clock, no
  I/O" becomes a property; a trap is caught where a segfault is not; one binary serves Linux and
  Windows. dlopen keeps its value for OpenFX, which is C ABI by definition.
- **The leaf rebuild is measured, not assumed (t977).** An edit of a plugin kernel recompiles only
  its family, the assembler, kt-app and kt-cli (`src/scripts/plugin-rebuild-check.py check`; red
  for an edit in kt-fx). The TEST build does not get cheaper: the assembler is a dev-dependency of
  seven core crates, whose ~100 integration test binaries relink (archived tasks 1.1 and 7.2).

## Deferred

- The barrier video kinds (blur, glow, edges, vhs, sharpen, pixelate, halftone, radial_blur,
  drop_shadow, chromatic_aberration, noise, greenscreen): `plugin-api-barriers`, with a GPU pass
  graph and gated SIMD twins.
- The wasm host (stage 2), then dlopen and the OpenFX bridge (stage 3).
- A plugin that ships UI: kind-specific widgets (the EQ graph, colour rows, camera handles) stay in
  kt-app, looked up by plugin id.
- The cost of the test build after a plugin edit (fewer test binaries per crate, or the
  registry-wide tests gathered where the set is needed).
