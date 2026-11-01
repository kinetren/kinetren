# kt-plugin-api

> The crate's contracts and mandatory tests — `openspec/specs/plugins/spec.md` (and the families'
> capabilities: `video-fx`, `audio-fx`, `transitions`, `generators`, `automation` for the passport);
> the reasons — `openspec/specs/plugins/design.md`.

The plugin contract: a `Manifest` (id, passport rows, declarations, UI keys) plus pure kernels per
family (`VideoKernel`/`GeoKernel`, `AudioKernel`, `TransKernel`, `GenKernel`), and the `Registry`
keyed by `(Family, id)`. Depends on kt-time, serde and libm ONLY: a plugin crate sees nothing of the core.

- **A plugin id is the kind's file tag**, unique within its family (`Registry::get(family, id)`):
  the generator `checker` is not the transition `checker` (t1049). Changing an id changes files.
- **Refusals happen in `Registry::register`, before any frame** (t972): video state, tail or
  look-ahead; a kernel not callable from several threads; an audio look-ahead reaching
  `MIN_PREROLL_FRAMES`; a built-in point-wise kernel without WGSL; a geometry kernel offering WGSL;
  a generator decl naming a row its passport lacks. For a barrier (t1054): no CPU kernel, no GPU
  side as a built-in, a tap's WGSL instead of a graph, a graph reading an unsaved slot, ending off the
  frame size or on a `Save`, a resample above `MAX_INTER_DIM`, a pass naming a missing shader, or an
  empty graph on a frame with alpha. The graph is checked on the defaults and the top, at K 1 and 4.
  A new declaration gets its refusal here too.
- **A `Stream` class is a SOURCE STAGE, not video state** (datamosh-codec-path): its
  `StreamKernel` names the HISTORY frame N reads and renders N as a pure function of it, keeping
  results only in the host-owned `StreamMemo` (bounded, keyed by addresses). Refused (t1240): the
  class without a stream kernel, a stream kernel on another class, any GPU side on a stream plugin.
- **Helpers shared by kernels and the host live here so each has ONE text**: `geo::invert3` and
  `TRANSPARENT`, `trans::Img`, the generator helpers (`smooth`, `rgb`, `coverage`, `lowbias32`…),
  `pixfont` (moved from kt-media). A helper the host has its own SIMD twin of stays in the host and
  is reached through a context (`barrier::RasterHost`, plugin-api-barriers).
- **`math` is the ONE math of the frozen paths** (ci-green-again, invariant 11): a kernel, a
  curve, a generator or the engine calls `x.fsin()`/`10f32.fpowf(…)` (`math::FrozenMath`), never
  `x.sin()`/`powf` — those are the PLATFORM's libm, and glibc and the Windows one round
  differently (t973c/t1019/t1048 failed on windows-latest). The `f32` functions are the `f64` ones
  rounded once (correctly rounded but for ~2⁻²⁸ of inputs); t1065 holds them within one ulp of
  glibc, t1066 refuses a direct std call in the frozen crates and every family.
- **A barrier sees all its rows through the values door** (camera-isp-simulation, D2):
  `BarrierKernel::frame_values`/`gpu_values` take the slot's values in passport order; their
  defaults are `frame`/`gpu` over `cook`. The HOST calls only the values forms (kt-fx
  `apply_barrier`, the kt-gpu barrier op, the registry's graph check — t1210); a barrier with more
  rows than the eight cooked numbers (camera_isp: ~85) implements them and answers its defaults in
  the cooked forms.
- The module is `generator`, not `gen`: `gen` is a reserved keyword in Rust 2024.
- Mandatory tests: t972 (`tests/registry.rs`) and every family's frozen-form test (t973,
  t1048) when a row or a declaration changes.
