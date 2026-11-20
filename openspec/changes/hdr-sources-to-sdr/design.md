# Design: hdr-sources-to-sdr

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
reader. That is Q1 b), not this change.

**Keys:** the swscale context's key gains the path (SDR, WIDE or HDR with its transfer and peak). The
burst cache's keys do not change: a source's frames at a quality are what this build decodes, as they
were before source-colour-matrix. The disk caches of decoded pixels move their versions, and the
render-cache disk tier is salted by the build.

## D4. The export default (Q3)

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

## D7. Stages and tests

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

Spare in this session's block: t3812–t3899.

## Risks

- **The look of the tone map is a taste.** The standard's anchors are the default, and the fixture
  frames are checked by eye before they are stored (task 4.1).
- **Cost:** a 4K HDR frame runs about seven table lookups and two small matrices per pixel on the
  processor, row-parallel inside the decoder. It is measured on release against the decoder's own time
  before it is merged (task 2.6).
- **A file with wrong tags** (an SDR file tagged HLG) is mapped wrongly. The facts show the path, and
  the per-file override is the follow-up.
