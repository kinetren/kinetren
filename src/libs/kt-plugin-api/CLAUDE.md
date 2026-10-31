# kt-plugin-api

> The crate's contracts and mandatory tests — `openspec/specs/plugins/spec.md` (and the families'
> capabilities: `video-fx`, `audio-fx`, `transitions`, `generators`, `automation` for the passport);
> the reasons — `openspec/specs/plugins/design.md`.

The plugin contract: a `Manifest` (id, passport rows, declarations, UI keys) plus pure kernels per
family (`VideoKernel`/`GeoKernel`, `AudioKernel`, `TransKernel`, `GenKernel`), and the `Registry`
keyed by `(Family, id)`. Depends on kt-time and serde ONLY: a plugin crate sees nothing of the core.

- **A plugin id is the kind's file tag**, unique within its family (`Registry::get(family, id)`):
  the generator `checker` is not the transition `checker` (t1049). Changing an id changes files.
- **Refusals happen in `Registry::register`, before any frame** (t972): video state, tail or
  look-ahead; a kernel not callable from several threads; an audio look-ahead reaching
  `MIN_PREROLL_FRAMES`; a built-in point-wise kernel without WGSL; a geometry kernel offering WGSL;
  a generator decl naming a row its passport lacks. A new declaration gets its refusal here too.
- **Helpers shared by kernels and the host live here so each has ONE text**: `geo::invert3` and
  `TRANSPARENT`, `trans::Img`, the generator helpers (`smooth`, `rgb`, `coverage`, `lowbias32`…),
  `pixfont` (moved from kt-media). A helper the host has its own SIMD twin of stays in the host and
  is reached through a context (the `plugin-api-barriers` design, `RasterHost`).
- The module is `generator`, not `gen`: `gen` is a reserved keyword in Rust 2024.
- Mandatory tests: t972 (`tests/registry.rs`) and every family's frozen-form test (t973,
  t1048) when a row or a declaration changes.
