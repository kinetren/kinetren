# Proposal: hdr-sources-to-sdr

> DESIGN ONLY (spec session). Code starts after the owner answers the questions at the end and the
> orchestrator gives the go. Tests reserved: t3800–t3811. Base: `orch-26-11-01` at `3420bf58`. Feature
> note: F-CLR-009 (`docs/features/color/management/hdr-sources-in-sdr-edit.md`).

## Why

- **found-defects 724:** wide-gamut and HDR sources are shown without gamut or tone mapping.
  source-colour-matrix reads the primaries and converts only the MATRIX. A BT.2020 / PQ / HLG source
  gets the BT.2020 matrix and is drawn as if its primaries were BT.709 and its transfer SDR: washed out
  and grey, in the preview and in the export. An iPhone films HLG BT.2020 10-bit HEVC by default, so
  this is ordinary phone footage, not a corner case. The 10 bits are also cut to 8 at the same step,
  before any mapping could be done.
- **found-defects 1054:** an HD export is written as BT.601 limited by default. The decode reads an
  untagged HD file as BT.709 by the size rule, and so do players that ignore tags. A file written by
  default thus disagrees with the program's own rule for reading it.

## What Changes

- **The transfer and the mastering levels are read.** `ColourTags` gains `transfer` (`AVColorTransfer`).
  The facts of a source gain the mastering display's peak and MaxCLL when the stream carries them.
- **An HDR or wide-gamut picture is mapped to the working space at decode.** The working space is BT.709
  SDR, 8-bit RGBA, as today. The decode runs one integer pipeline:
  1. swscale scales and upsamples the planes to 16-bit 4:4:4 at the decode size (no RGB conversion);
  2. Q15 matrix to non-linear RGB;
  3. EOTF (PQ, or HLG with its OOTF) by table to linear light;
  4. BT.2020/P3 → BT.709 primaries in linear light;
  5. tone map: BT.2408 reference white, BT.2390 highlight roll-off by table;
  6. out-of-gamut colours brought in toward their luminance;
  7. BT.709 SDR encoding by table to 8 bits.

  The tables are built once from `kt_plugin_api::math` (frozen), so the bytes are the same on every
  CPU.
- **The same pass runs on the device.** Once the device decode exists (device-decode-sources,
  F-MED-010), it runs the same integer pass in WGSL, byte-equal to the processor's.
- **SDR is untouched.** A source tagged SDR BT.709/BT.601, or untagged, keeps today's swscale
  conversion byte for byte. GOLDEN_VIDEO and every stored frame stay.
- **The export default follows the decode's size rule (Q3):** BT.709 at HD and above, BT.601 below,
  limited range, tagged as it is converted.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: the decode maps HDR and wide-gamut pictures to the BT.709 SDR working space.
- `export`: the default matrix follows the size rule.

## Impact

- **kt-ffmpeg:**
  - `colour.rs`: `transfer`, the HDR resolve, the integer pipeline and its tables;
  - `source_impl.rs`: the HDR conversion branch, with the sws context's key carrying it;
  - `format.rs`: the export default.
- **kt-media:** the facts `MediaInfo` carries (transfer, mastering peak).
- **kt-gpu:** the WGSL twin, which arrives with the device decode.
- **Disk caches** of decoded pixels move their versions (`THUMB_PIXELS_VERSION`, `STRIP_ROW_VERSION`),
  as source-colour-matrix did. The render-cache disk tier is salted by the build already.
- **Invariants:**
  - 4: unchanged; the cache still holds pre-FX RGBA;
  - 7: one pipeline, two executors byte-equal by integer arithmetic;
  - 10: `color_trc` and the side data are read through the existing per-major modules, with no new
    required symbol;
  - 11: integer arithmetic and frozen tables, no platform libm.

## Questions for the owner

**Q1. What does the program compute in: SDR BT.709 8-bit with HDR mapped at decode, or a linear
high-precision working space?**

Your project shows phone footage from the same device next to each other. Today HDR clips are grey and
washed out (found-defects 724), and nothing converts them.

- **a) SDR BT.709 8-bit, HDR mapped at decode** (this change). Every effect, cache and kernel stays as it
  is. *(Recommended: it fixes what you see now at L cost; the other path stays open.)*
- **b) A linear-light 16-bit or float working space** (Resolve's colour management, FCP's Wide Gamut
  library). XL: every kernel, cache and memo changes, and memory grows 2–4×. It is the prerequisite of
  real HDR output.

**Q2. HDR OUTPUT: later, as its own change, or not planned?**

Today every export is SDR 8-bit. HDR output (PQ/HLG 10-bit with mastering metadata) needs Q1 b) under it.

- **a) Later**, recorded in F-CLR-008 as the next step after Q1 b). *(Recommended: no one has asked for
  it.)*
- **b) Not planned.**

**Q3. The export's default matrix (found-defects 1054).**

Today every export is converted and tagged BT.601 limited. The file is correct for players that read
tags. A player that ignores them, and our own decode for an untagged file, assume BT.709 at HD, and show
such a file with shifted colours.

- **a) Follow the decode's size rule:** BT.709 at 1280 wide or taller than 576, BT.601 below.
  *(Recommended: one rule for reading and writing, and what most HD deliveries use.)*
- **b) Always BT.709.**
- **c) Keep BT.601** for everything. A choice in the render window stays in every option.
