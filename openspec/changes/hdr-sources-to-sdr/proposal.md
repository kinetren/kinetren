# Proposal: hdr-sources-to-sdr

> DESIGN (spec session). Revised 2026-11-20 on the owner's answer (kkt-26.11.20 A25, line 33): the
> project's or composition's setting decides between a 16-bit float path and today's fast 8-bit path.
> Code starts on the orchestrator's go after his answers to Q1–Q4 below. Tests reserved: t3800–t3811
> (the 8-bit path, unchanged numbers), t3877–t3899 (the 16-bit path). Base: `orch-26-11-01` at `5e7a8ea9`.
> Feature note: F-CLR-009 (`docs/features/color/management/hdr-sources-in-sdr-edit.md`).

## Why

- **found-defects 724:** wide-gamut and HDR sources are shown without gamut or tone mapping.
  source-colour-matrix reads the primaries and converts only the MATRIX. A BT.2020 / PQ / HLG source
  gets the BT.2020 matrix and is drawn as if its primaries were BT.709 and its transfer SDR: washed out
  and grey, in the preview and in the export. An iPhone films HLG BT.2020 10-bit HEVC by default, so
  this is ordinary phone footage, not a corner case. The 10 bits are also cut to 8 at the same step.
- **found-defects 1054:** an HD export is written as BT.601 limited by default; the owner chose BT.709
  from HD up (kkt-26.11.20 A3, executor mosh-sharp; design D4 stays the rule).
- **The owner's decision** (Q48/Q49, kkt-26.11.20 line 33): "нужно от настроек проекта-композиции
  определять, если включено то тракт f16 чтобы было нормально, если нет - то быстрый i8". So both:
  - **off** (the default): today's fast 8-bit path, with HDR sources mapped to SDR at decode so they
    are no longer grey (the design's D1–D7);
  - **on**: a 16-bit float path from decode to output, in which an HDR picture keeps its highlights and
    its 10 bits, and is shown and exported right (D8–D15).

## What Changes

- **A precision setting on the project and on every composition:** "Picture: 8-bit (fast)" or "16-bit
  float (HDR)".
  - A composition (reel) left unset takes the project's.
  - It is written in the `.kt` only when it is 16-bit, so old files do not change.
- **8-bit path:** as designed before. The decode reads the transfer and the mastering levels, and maps
  HDR and wide-gamut pictures to BT.709 SDR 8-bit by one integer pipeline:
  - EOTF, gamut, the BT.2408/BT.2390 tone map, out-of-gamut toward luminance, BT.709 encode;
  - its tables come from `kt_plugin_api::math`;
  - SDR sources keep today's bytes.
- **16-bit path:**
  - **The working space** is scene-linear light in BT.2020 primaries, f16 RGBA, with 1.0 = 203 cd/m², the
    HDR reference white of BT.2408 (Q1).
  - **The decode** converts every source into it by the same integer pipeline up to linear light, with no
    tone map. An SDR source goes through its own curve.
  - **The frames cache, layer textures and memos** are `Rgba16Float`: 8 bytes a pixel, which the memory
    ledger counts.
  - **The core passes run in float:** geometry, resample, layer mix, the crossfade, masks, premultiply
    and the composite.
  - **An effect declares its precision.** One without a float kernel runs in an 8-bit island: its input
    is the SDR rendition, its output is expanded back, and the chain shows an "8-bit" mark on it (Q2).
  - **The display pass** tone-maps the linear picture to the monitor: BT.2390 to SDR on an SDR surface,
    and HDR on an HDR surface where the system gives one (Q3).
  - **The export** writes SDR (tone-mapped, as the display), or HDR10 PQ, or HLG, 10-bit, with mastering
    metadata, on the encoders that take 10 bits (Q4).
- **The boundary between precisions:** a 16-bit composition inside an 8-bit project is tone-mapped to SDR
  at its boundary; an 8-bit one inside a 16-bit project is expanded to linear.
- **Stream stages** (Datamosh) are codecs of 8-bit pictures by nature. They are 8-bit islands, marked.
- **The prerender** of a 16-bit project stores HEVC Main10 PQ segments where a 10-bit hardware encoder
  opens, and nothing where none does (the facts say why).

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: the decode reads the transfer and maps by the project's precision.
- `video-plan`: the precision of a project and a composition; the 16-bit path's formats, memory and
  boundaries; the display's tone map; the prerender's 10-bit segments.
- `video-fx`: an effect declares its precision; the 8-bit island and its mark.
- `export`: the default matrix by size; SDR, HDR10 and HLG from a 16-bit project.
- `project-format`: the precision field, a new file format by content.
- `ui-reach`: the precision in Project settings and in a composition's settings; the island's mark.

## Impact

- **kt-ffmpeg:** `colour.rs` (`transfer`, the mastering levels, the integer pipeline, its tables and its
  f16 output); `source_impl.rs` (the 16-bit conversion branch); `format.rs` and `muxer_impl.rs` (10-bit
  HDR tags, mastering display and content light SEI).
- **kt-media:** `VideoFrame` gains a pixel format (`Rgba8` | `Rgba16F`); the facts carry transfer and
  peak.
- **kt-cache:** byte accounting is already by length; the ledger's frames budget holds half as many
  16-bit frames.
- **kt-gpu:**
  - the pool's format per plan (`Rgba8Uint` | `Rgba16Float`), with `tex_bytes` by format;
  - float twins of the core shaders, the display's tone map and HDR surface;
  - `FrameTol` in float units.
- **kt-plugin-api:** `Precision` in the manifest; float kernels for the effects that have them.
- **kt-engine:** the plan carries the precision; the island's conversions; export's 10-bit HDR from f16;
  the prerender's Main10 segments.
- **kt-model:** `ProjectSettings.precision`, `Sequence.precision: Option<_>`; the next free project file format by
  content.
- **kt-app:** the settings' control, the chain's mark, the display surface.
- **kt-otio:** the precision goes into `metadata["kt"]`; a row in "What OTIO export loses".
- **Invariants:**
  - 4: the cache key gains the precision;
  - 7: one plan, its precision a field;
  - 11: the 8-bit path stays byte-exact everywhere. The 16-bit path is byte-exact between runs on one
    device and within float tolerances across devices; its decode is integer, so frames enter equal.

## The owner's answers taken

- **Q48/Q49 (HDR working space and output):** "нужно от настроек проекта-композиции определять, если
  включено то тракт f16 чтобы было нормально, если нет - то быстрый i8". Both paths are built, chosen per
  project and per composition. The earlier Q1 (8-bit with mapping, or a linear float space) is answered
  "both, by the setting". The earlier Q2 (HDR output later) is answered by the 16-bit path: its export
  writes HDR (Q4 below names the forms).
- **Q50 (export default):** "709 от HD, 601 ниже" (design D4; found-defects 1054, executor mosh-sharp).

## Questions to the owner

**Q1. What is "1.0" in the 16-bit path?**
In the 16-bit path every picture is light in linear numbers. One number has to mean "the white of a
normal SDR picture".
- **a) 1.0 = 203 cd/m², linear light, BT.2020 colours** (the HDR reference white of BT.2408; FCP's Wide
  Gamut HDR and Resolve's HDR modes work this way). An SDR clip's white is 1.0, an HDR highlight 5.0 or
  more. *(Recommended: effects written for "white = 1" keep working on SDR material, and HDR has room
  above.)*
- **b) 1.0 = 10 000 cd/m²** (absolute PQ scale). SDR white is 0.02, which is too small for effects tuned
  for 0..1.

**Q2. An effect that has no 16-bit version yet, in a 16-bit project.**
Today every effect is written for 8 bits (about 50 picture kernels). The core (geometry, mixing,
transitions, masks) gets float versions in this change, and the colour effects and blurs first after it.
- **a) It runs on the SDR version of the picture and is marked "8-bit" in the chain.** HDR highlights
  above white are clipped through it, and the mark says so. *(Recommended: every effect stays usable,
  and you see which ones cost the HDR.)*
- **b) It is greyed out in a 16-bit project** until its float version exists.

**Q3. Showing HDR on the monitor.**
"чтобы было нормально": in the 16-bit path the preview tone-maps the picture to your monitor.
- **a) Tone-mapped to SDR on every monitor now**, highlights rolled off instead of clipped. A real HDR
  monitor output (Windows HDR, Wayland HDR) follows, where the graphics library gives an HDR surface.
  *(Recommended: it looks right everywhere at once; HDR surfaces are not yet reliable on Linux.)*
- **b) HDR output to an HDR monitor in this change**, where the system supports it.

**Q4. What can a 16-bit project export?**
- **a) SDR (tone-mapped as the preview shows it), HDR10 (PQ) and HLG, all 10-bit,** on the encoders that
  take 10 bits: x265, x264 High10, SVT-AV1, hevc_nvenc, ProRes. *(Recommended: the reason for the 16-bit
  path is to deliver HDR.)*
- **b) SDR only** for now; HDR export later.
