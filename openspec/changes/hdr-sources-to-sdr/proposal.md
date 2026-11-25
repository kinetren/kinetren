# Proposal: hdr-sources-to-sdr

> **The 8-bit path (stages 1–4) is IMPLEMENTATION-READY** (spec-engine, revision 3, 2026-11-24, orch `229c694b`); the 16-bit path (stage 5) is implementation-ready
> too (revision 4, 2026-11-25, orch `fc60a5d9`), as six sessions after the rows that write the same shaders. The owner answered (kkt-26.11.20 A25, line 33; kkt-26.11.20b B4). Feature note: F-CLR-009.

## Why

- **found-defects 724:** wide-gamut and HDR sources are shown without gamut or tone mapping.
  source-colour-matrix reads the primaries and converts only the MATRIX. A BT.2020 / PQ / HLG source
  gets the BT.2020 matrix and is drawn as if its primaries were BT.709 and its transfer SDR: washed out
  and grey, in the preview and in the export. An iPhone films HLG BT.2020 10-bit HEVC by default, so
  this is ordinary phone footage, not a corner case. The 10 bits are also cut to 8 at the same step.
- **found-defects 1054:** an HD export is written as BT.601 limited by default; the owner chose BT.709
  from HD up (kkt-26.11.20 A3). The mosh-sharp executor built it (t3603 in `export/spec.md`), so this
  change no longer modifies that requirement.
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
  - EOTF, the BT.2390 tone map (peak → 203 cd/m², reference white at code 230 for a 1000 cd/m² master, so highlights
    have codes left), gamut, out-of-gamut toward luminance, SDR encode;
  - its tables come from `kt_plugin_api::math`;
  - the peak is asked once, from the stream's first picture; only mapped sources get new names;
  - SDR sources keep today's bytes.
- **16-bit path:**
  - **The working space** is scene-linear light in BT.2020 primaries, f16 RGBA (straight alpha), with 1.0 = 203 cd/m², the
    HDR reference white of BT.2408 (the owner's Q1 a).
  - **The decode** converts every source into it by the same integer pipeline up to linear light, with no
    tone map. An SDR source goes through its own curve.
  - **The frames cache, layer textures and memos** are `Rgba16Float`: 8 bytes a pixel, which the memory
    ledger counts.
  - **The core passes run in float:** geometry, resample, layer mix, the crossfade, masks, premultiply
    and the composite.
  - **A point-wise effect may declare that it goes beyond white** (colour grade, vignette): it runs on the same text with no clamp. Any other runs in an 8-bit island: its input
    is the SDR rendition, its output is expanded back, and the chain shows an "8-bit" mark on it (the owner's Q2 a).
  - **The display pass** tone-maps the linear picture to the monitor: BT.2390 to SDR on an SDR surface,
    and HDR on an HDR monitor at once wherever the system gives an HDR surface (the owner's Q3 b).
  - **The export** writes SDR (tone-mapped, as the display), or HDR10 PQ, or HLG, 10-bit, with mastering
    metadata, on the encoders that take 10 bits (the owner's Q4 a).
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

- **kt-ffmpeg:** `colour.rs` (`transfer`, the path), new `hdr.rs` (the integer pipeline, its tables; later its
  f16 output), a dependency on kt-plugin-api (the frozen math); `source_impl.rs` (the 16-bit conversion branch); `format.rs` and `muxer_impl.rs` (10-bit
  HDR tags, mastering display and content light SEI).
- **kt-media:** `MediaSource::colour`, a mapped source's identity, the copy's name; with stage 5 `VideoFrame` gains a pixel
  format (`Rgba8` | `Rgba16F`).
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
  writes HDR (the owner's Q4 a: SDR, HDR10 and HLG).
- **Q50 (export default):** "709 от HD, 601 ниже" (design D4; found-defects 1054, executor mosh-sharp).

## The owner's answers to the revised questions (kkt-26.11.20b, lines 8–10, row B4)

- **Q1 a — "1.0 = белый SDR, 203 кд/м² (Recommended)".** Design D9.
- **Q2 a — "Работает, помечен «8 бит» (Recommended)".** An effect without a float kernel runs as a marked
  island (D12).
- **Q3 b and Q4 a — "HDR на HDR-монитор сразу".**
  - The preview presents HDR on an HDR monitor in this change, wherever the system gives an HDR surface,
    and tone-maps to SDR elsewhere (D13).
  - The export writes SDR, HDR10 and HLG (D14).
