# Design: hdr-sources-to-sdr

> Revised 2026-11-20 (kkt-26.11.20 A25): the project's or composition's precision decides. D1–D7 are the
> **8-bit path** (the default, "быстрый i8"); D8–D15 are the **16-bit float path** ("тракт f16").

## Context

- **source-colour-matrix** (`kt-ffmpeg/src/colour.rs`, media-index "A source's YUV becomes RGB with its
  own matrix and range"):
  - `ColourTags { matrix, range, primaries }` come from the picture, else the stream.
  - `resolve` picks the swscale matrix and range.
  - `to_rgba` in `source_impl.rs` converts with `sws_getContext(..., SWS_FAST_BILINEAR)` to 8-bit RGBA at
    the decode size, keyed by matrix and range.
  - Primaries are read and reported, not converted. The transfer is not read at all.
- **Precision:** a 10-bit source (`yuv420p10le`, P010 from a device) is converted straight to RGBA8. A
  PQ signal keeps about 50 of its 1024 code values below 100 cd/m², so any tone map AFTER this step works
  on a few dozen levels.
- **The working space** is BT.709 SDR 8-bit RGBA everywhere after decode: the burst cache, the kernels,
  the memos, the export's RGB→YUV (F-CLR-008: no working-space decision has been taken).
- **The export** (`kt-ffmpeg/src/format.rs`): `ExportOptions::default` has `matrix: Matrix::Bt601`,
  limited range, BT.709 primaries and transfer tags. Its comment's reason ("what the preview's decode
  does") is stale since source-colour-matrix (found-defects 1053).
- **Device decode** (device-decode-sources, designed): device pictures (NV12/P010) are laid out
  byte-equal to the processor decoder's planes, then take the same conversion.

## D1. Which pictures are mapped

A picture takes the HDR path when its resolved tags say it is not BT.709/BT.601 SDR:

| transfer (`AVColorTransfer`) | primaries | path |
|---|---|---|
| SMPTE ST 2084 (PQ, 16) | any | HDR: PQ EOTF, gamut, tone map |
| ARIB STD-B67 (HLG, 18) | any | HDR: HLG inverse OETF + OOTF (system gamma 1.2 at 1000 cd/m²), gamut, tone map |
| BT.709 / BT.601 / BT.2020-10/12 (SDR curves) / unspecified | BT.2020 (9) or P3-D65 (12) | WIDE: SDR curve, gamut only |
| same SDR curves | BT.709 / BT.601 / unspecified | today's swscale path, byte for byte |

An unknown transfer falls to the SDR rule and says so in the source's facts. A tag override per FILE
(F-CLR-008 trick 2) is a follow-up; the facts name what was read so an override has something to show.

## D2. The integer pipeline (one definition, two executors)

Input: the picture's planes. Output: RGBA8 at the decode size. Every stage is integer or a table lookup.

1. **Scale and upsample on the processor:** swscale converts the planes to `yuv444p16` at the decode
   size (the same scaler and flags as today, no matrix). Chroma siting comes from the tag
   (`chroma_location`), and swscale applies it.
2. **Matrix:** BT.2020 non-constant-luminance (or the tagged matrix), limited or full range, Q15
   integer coefficients → R′G′B′ in 16 bits, clamped.
3. **EOTF table:** 4096 entries indexed by the top 12 bits, linearly interpolated by the low 4, in Q16.
   - PQ: absolute luminance / 10000 cd/m².
   - HLG: the inverse OETF and the BT.2100 OOTF at the nominal peak (1000 cd/m², γ = 1.2), applied on
     the luminance and expressed in the same absolute scale.
4. **Gamut matrix** in linear light, BT.2020 → BT.709 (or P3-D65 → BT.709), Q14 signed. The result may
   be negative or above 1.
5. **Tone map**, a 4096-entry table on the maximum of R, G, B (hue and saturation kept by scaling all
   three by the same ratio):
   - the HDR reference white (203 cd/m², BT.2408) maps to the SDR reference white;
   - highlights roll off by the BT.2390 EETF from the source peak (mastering MaxCLL or the mastering
     display's peak when present, else 1000 cd/m²) to the SDR peak;
   - below the knee the map is linear.
   - The anchors are a table of constants in `colour.rs`, checked by eye on the fixtures (task 4.1).
6. **Out of gamut:** a negative channel is brought in by mixing the pixel toward its BT.709 luminance by
   the least amount that makes every channel ≥ 0 (integer, per pixel). A channel above 1 is clipped after
   the tone map.
7. **Encode:** the BT.709 SDR curve (the inverse of BT.1886, γ 2.4) by a 4096-entry table to 8 bits.
   Alpha is 255.

The WIDE path is stages 1, 2, 3 with the SDR curve's EOTF, 4, 6 and 7, with no tone map.

The tables are built once per process from `kt_plugin_api::math` (frozen, t1066): the same bits on every
CPU and OS. The processor executor is a row-parallel Rust kernel over the `yuv444p16` planes, inside the
decoder's own thread (invariant 5). The device executor is the same integer arithmetic in a WGSL
compute pass, with the tables as storage buffers. It arrives with device-decode-sources and is held
byte-equal to the processor (t3807).

## D3. Where it sits, and why not in the plan

F-CLR-008 wanted conversions "in the plan, as a CPU + WGSL pair". This half cannot be there: the plan
reads the burst cache, which holds RGBA8 (invariant 4). A tone map after that quantization bands
(Context). The decode is where the source's precision still exists, and the matrix conversion is already
there. Moving the cache to 16 bits for HDR sources only would double their cache bytes and touch every
reader. That is the 16-bit path (D8–D15), chosen per project by the owner (kkt-26.11.20 A25).

**Keys:** the swscale context's key gains the path (SDR, WIDE or HDR with its transfer and peak). The
burst cache's keys do not change: a source's frames at a quality are what this build decodes, as they
were before source-colour-matrix. The disk caches of decoded pixels move their versions, and the
render-cache disk tier is salted by the build.

## D4. The export default (the owner's Q50: "709 от HD, 601 ниже")

`ExportOptions::default` takes its matrix from the output size by `colour::is_hd`: BT.709 at HD and
above, BT.601 below. The range stays limited. The tags are written as converted (t2806's law holds). The
render window still offers both. The text form (`export_settings`) writes `matrix=auto` for the default,
so a preset made before this change, which stored `bt601`, keeps it.

## D5. Facts and telemetry

- `MediaInfo` gains `transfer`, `primaries` (already read) and `hdr_peak: Option<u32>` (cd/m²).
- `kt-cli probe` prints the path a source takes ("HDR PQ → SDR, peak 1000", "wide gamut BT.2020 → 709").
- The trace's decode record names the path, so a "my clip looks washed out" complaint is answered from
  the owner's trace (telemetry rule).

## D6. Class walk: every place a picture's colour is decided or written

| Place | Today | After |
|---|---|---|
| processor decode `to_rgba` | matrix + range | + the WIDE/HDR path (D1, D2) |
| device decode (device-decode-sources) | planes laid out, same conversion | the WGSL twin of D2 |
| thumbnails, reel strips, posters | the same decode | the same; versions moved |
| still images (PNG/JPEG with ICC or wide gamut) | decoded as sRGB | unchanged; an ICC profile is a follow-up (F-CLR-008) |
| generators, titles | drawn in the working space | unchanged |
| the render-cache segments (H.264, BT.709 tags) | working-space pictures | unchanged |
| the export RGB→YUV and tags | BT.601 default | the size rule (D4) |
| proxies (spec-features `proxies`) | a proxy written from the decode | written from the mapped picture, tagged BT.709 SDR: a proxy of an HDR source is SDR, and the design notes it |
| scopes (spec-features `scopes`) | read the working space | unchanged: they show the mapped picture |
| LUTs (F-CLR-004) | applied in the working space | unchanged: a LUT made for HLG input sees the mapped picture; a "no mapping" override is the follow-up |

## Risks (the 8-bit path)

- **The look of the tone map is a taste.** The standard's anchors are the default, and the fixture
  frames are checked by eye before they are stored (task 4.1).
- **Cost:** a 4K HDR frame runs about seven table lookups and two small matrices per pixel on the
  processor, row-parallel inside the decoder. It is measured on release against the decoder's own time
  before it is merged (task 2.6).
- **A file with wrong tags** (an SDR file tagged HLG) is mapped wrongly. The facts show the path, and
  the per-file override is the follow-up.

## D8. The precision setting

- `ProjectSettings.precision: Precision { Rgba8, Rgba16F }` with default `Rgba8`. `Sequence.precision:
  Option<Precision>`, where `None` means the project's.
- A plan is built for one precision: the frame's own (the root's, or the reel's for its sub-plan). The
  precision is part of the plan key and of the burst cache key (invariant 4: the bars ask for the
  precision the plan reads).
- Changing it is an edit: undo returns it; the player rebuilds as for any settings edit; caches of the
  other precision stay until evicted.
- **The file:** the field is written only when it is `Rgba16F`. Such a file is project-file format 8 by
  content (`version_for`; an old reader refuses it, naming the version, as the format rules require). An
  8-bit project is byte-identical to before.
- **The OTIO door** writes it into `metadata["kt"]` and adds a row in "What OTIO export loses" (other
  editors read an 8-bit timeline).

## D9. The working space of the 16-bit path (Q1)

- Scene-linear light in BT.2020 primaries, `Rgba16Float`, premultiplied alpha, with 1.0 = 203 cd/m² (BT.2408
  HDR reference white).
- An SDR source is converted to linear through BT.1886 and its own primaries: SDR white becomes 1.0.
- A PQ source maps its absolute luminance by `L / 203`. A 1000 cd/m² highlight becomes 4.93; f16 holds up
  to 65504 and keeps 10 bits of mantissa, more than a 10-bit source's steps at every level.
- An HLG source is converted with the BT.2100 OOTF at the nominal 1000 cd/m², then scaled likewise.

## D10. The decode

The D2 pipeline runs on every picture of a 16-bit plan up to stage 4 (gamut to BT.2020 instead of
BT.709). Then:
- the result is scaled to the working white (D9);
- it is written as f16 by an exact integer conversion: a table from Q16 linear to the f16 bit pattern,
  with round-to-nearest-even done in integers.

The decode stays integer and table-driven, so a 16-bit frame enters the cache with the same bits on
every processor (invariant 11). `VideoFrame` gains `format: PixFormat { Rgba8, Rgba16F }`; the bytes are
`w·h·8` for f16. The frames budget of the memory ledger is in bytes and holds half as many frames.

## D11. The GPU executor

- **The pool format** is chosen by the plan's precision: `Rgba8Uint` (today) or `Rgba16Float`.
  `tex_bytes = w·h·bytes_per_pixel(format)`, so the pool, the upload cache, the present memo and the
  readback cache count by format.
- **The core shaders** get float twins, chosen when the pipeline is compiled: composite, layer_mix,
  premul/unpremul, resample, mirror, mask, grid_tile, geom_fused, the crossfade and yuv (export).
  `common.wgsl` gains `load_px`/`store_px` over `texture_2d<f32>`. The 8-bit shaders are unchanged
  (invariant 11: their bytes do not move).
- **`FrameTol`** gains float classes in units of 2⁻¹⁰ relative error, applied to f16 frames on two
  devices (the software adapter and a card).

## D12. Effects: a declared precision and the 8-bit island (Q2)

- `Manifest.precision: &[Precision]` lists the precisions an effect's kernel has. Built-in effects start
  with `[Rgba8]`.
- This change gives float kernels to: Geometry, CamMotion, LayerMix, the crossfade, the title, and the
  colour family (`color_grade`, curves, levels, LUT). The blurs follow in the first round after it.
- **An effect without a float kernel in a 16-bit chain runs as an island:**
  1. the input passes through the display's SDR tone map to `Rgba8Uint` (the same D13 curve);
  2. the 8-bit kernel runs;
  3. the output is expanded back through the curve's inverse below SDR white. Above white, the input's
     own highlight ratio is reapplied where the effect left the pixel's chroma unchanged; elsewhere it
     is clipped at 1.0.

  The chain row shows "8-bit" with a tooltip: "HDR highlights are limited through this effect".
- Islands next to each other share one conversion. The plan fuses them as it fuses point-wise runs.
- **Stream stages** (Datamosh, VHS Tape's tape, the Camera ISP's sensor path) model 8-bit or 10-bit
  devices. They are islands by nature and are marked the same. A 10-bit codec simulation is a later
  change.

## D13. The display (Q3)

- The display pass reads the f16 composite and tone-maps it by BT.2390 from the frame's peak to the
  monitor's SDR white (the same anchors as D2 stage 5, in float). It writes `Rgba8Unorm` as today.
- Where eframe/wgpu give an HDR surface (`Rgba16Float` with an extended or PQ colour space; Windows today,
  Wayland's colour management when it lands), a later step presents linear scRGB without the tone map.
  The display's facts say which.
- The scopes (change `scopes`) read the f16 composite, so they show nits above 100.

## D14. Export from a 16-bit project (Q4)

- **SDR:** the display's tone map, then today's RGB→YUV at 8 or 10 bits.
- **HDR10:** linear → BT.2020 PQ (inverse EOTF table), then the 10-bit Q15 matrix to `yuv420p10` / P010 /
  `yuv422p10`. Tags: BT.2020 matrix, primaries and SMPTE 2084 transfer. Mastering display (BT.2020, 1000
  cd/m² default) and MaxCLL/MaxFALL measured over the export, written by a second pass of the muxer's
  header (MKV) or by the encoder's SEI options (x265 `master-display`, `max-cll`).
- **HLG:** the BT.2100 inverse OOTF and OETF, tagged ARIB STD-B67.
- **Encoders** that take 10 bits: libx265, libx264 (High10), libsvtav1, libaom-av1, libvpx-vp9,
  hevc_nvenc, av1_nvenc, prores_ks. The render window offers HDR only with one of them. An 8-bit-only
  encoder offers SDR.
- The `yuv.rs` and `yuv.wgsl` conversions gain an f16 input, integer from the f16 bits by table (the
  export's bytes stay exact on one device).

## D15. Boundaries, the prerender and the class walk

**A composition inside another of a different precision:**
- an f16 reel in an 8-bit parent is tone-mapped (D13) at its boundary;
- an 8-bit reel in an f16 parent is expanded to linear (D10's SDR rule).

A reel preview strip is always 8-bit, tone-mapped.

**The prerender** (render cache) of a 16-bit plan writes HEVC Main10 PQ segments by the hardware cascade.
Where no 10-bit hardware encoder opens, it writes nothing, and the Diagnostics window says why.

| Place | 8-bit path | 16-bit path |
|---|---|---|
| decode | D1–D3: matrix, and HDR/WIDE mapped to SDR 8-bit | D10: linear BT.2020 f16 |
| frames cache | RGBA8 | RGBA16F (8 B/px), the precision in the key |
| core shaders | u32 0..255 | float twins (D11) |
| effects | as today | float kernels where declared, else an 8-bit island (D12) |
| Datamosh, tape, ISP | as today | an 8-bit island, marked |
| generators, titles | RGBA8 | the title in float; the other generators drawn in 8 bits and expanded |
| reels | as today | the boundary rule (D15) |
| display | `/255` to Rgba8Unorm | BT.2390 tone map; HDR surface later (D13) |
| scopes | the 8-bit composite | the f16 composite in nits |
| export | RGB→YUV 8/10 from 8 bits; BT.709 from HD (D4) | SDR / HDR10 / HLG 10-bit (D14) |
| prerender segments | H.264 8-bit | HEVC Main10 PQ, or none |
| thumbnails, reel strips, posters | 8-bit | 8-bit, tone-mapped |
| memory ledger | 4 B/px | 8 B/px in every tenant (`tex_bytes` by format) |
| durable keys, render index | as today | the precision in the key |
| project file | unchanged | format 8 when 16-bit |
| OTIO | unchanged | `metadata["kt"]`, a row in "What OTIO export loses" |

## D16. Stages and tests

1. Tags:
   - `transfer` and the mastering peak read on every major (t3800);
   - the path resolve (t3801).
2. The pipeline on the processor:
   - an SDR source is byte for byte as before, GOLDEN_VIDEO unchanged (t3802);
   - PQ and HLG fixtures (generated by `fixtures/gen.sh` with tags) map to stored frames, identical on
     two CPUs' tables and in a scalar and a row-parallel run (t3803);
   - a 10-bit gradient keeps more than 200 distinct output levels across its SDR range (no banding,
     t3804);
   - a saturated BT.2020 green maps inside BT.709 with its hue within 3° (t3805);
   - the WIDE path for P3 (t3806).
3. The device twin equals the processor byte for byte (t3807), when device-decode-sources lands.
4. Export:
   - the default by size, with tags (t3808);
   - an old preset keeps BT.601 (t3809).
5. Facts and `kt-cli probe` print the path (t3810); the trace names it (t3811).

6. The 16-bit path (after the owner's Q1–Q4):
   - the precision setting, the file's format 8, OTIO (t3877–t3880);
   - the decode to linear f16, exact on every processor (t3881–t3883);
   - the pool by format, the core float shaders, `FrameTol` float classes (t3884–t3887);
   - the effect's precision, the island and its mark, the colour family's float kernels (t3888–t3891);
   - the display's tone map (t3892);
   - the export SDR / HDR10 / HLG with their tags and mastering metadata (t3893–t3895);
   - the reels' boundaries and the prerender's Main10 segments (t3896, t3897);
   - the settings' control and the chain's mark (t3898); the idle check on a 16-bit heavy showcase (t3899).

## Risks (the 16-bit path)

- **Memory doubles** for every picture of a 16-bit project. The ledger's budgets stay in bytes, so fewer
  frames fit; the trace shows it.
- **Effects in islands** lose highlights. The marks show where, and the float kernels follow by family.
- **HDR surfaces** are uneven across systems. The tone-mapped SDR display is the path that always works.
