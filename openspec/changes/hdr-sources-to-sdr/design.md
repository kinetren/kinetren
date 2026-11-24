# Design: hdr-sources-to-sdr

> **D1–D7, the 8-bit path (the default, "быстрый i8"), are IMPLEMENTATION-READY** (spec-engine, revision 3, 2026-11-24, orch `229c694b`,
> found-defects 724). D8–D15, the 16-bit path ("тракт f16", kkt-26.11.20 A25), are a design of its own stage (5): it waits for every GPU,
> STREAM and COLOR-FX row and is re-read then.

## Context (what exists)

- `kt-ffmpeg/src/colour.rs` (pure): `ColourTags { matrix, range, primaries }`, `or`, `resolve` → `SourceColour`. The transfer is not read.
- `source_impl.rs` (included once per major): `stream_colour_tags`, `conversion_colour(api, frame, stream_tags)`, `Rgba::convert`: one swscale context
  (`SWS_FAST_BILINEAR`) to RGBA8 at the decode size, keyed by `SwsKey` (size, format, matrix, range); `None` from `conversion_colour` leaves swscale untouched. `span_sizes_threaded` runs
  several `Rgba` at once on `rgba_threads` threads; a decoder keeps its own.
- **Precision**: a 10-bit picture is converted straight to RGBA8. PQ spends about 50 of 1024 codes below 100 cd/m² on those 8 bits: a map after this step bands.
- **Device decode** (device-decode-sources D1): a device picture is laid out as the processor's planes on the HOST and takes the same `convert`. So there is one
  place; the WGSL twin an earlier revision planned has nothing to do and is cut.
- **Names**: `FfmpegSource::ident` is the file's identity (durable-moment-key), compared in `cursor_for` with the file that lies there (row 1065); `Made.pixels` is
  `THUMB_PIXELS_VERSION`.
- The export's default matrix is by size (row 1054, t3603).

## D1. Which pictures are mapped

`ColourTags` gains `transfer` (`AVColorTransferCharacteristic`, unspecified = 2; `or` fills it from the stream; t2891's numbers on every major). `resolve_path(tags, kind) -> Path`:

| transfer | primaries | path |
|---|---|---|
| 16 (PQ) | any | `Pq { peak }` |
| 18 (HLG) | any | `Hlg` |
| 1, 6, 14, 15 (the video SDR curves), 2 (unspecified), 13 (sRGB) | 9 (BT.2020) or 12 (P3-D65) | `Wide { curve: Video or Srgb, from }` |
| the same | anything else | `Sdr`: today's swscale path, byte for byte |
| any other transfer | any | `Sdr`, and the facts name the number read |

PQ/HLG with unspecified primaries are read as BT.2020. 11 (DCI-P3 with its own white) stays `Sdr`, named. `kind` is YUV or RGB (`is_yuv_format`, the descriptor's RGB flag): grey and palette pictures stay `Sdr`.
A picture's own tag wins over the stream's, as the matrix does.

## D2. The peak of a PQ stream is asked once, from its first picture

MaxCLL, else the mastering display's maximum, else 1000 cd/m². They ride on pictures as side data (`av_frame_get_side_data`, in every major: the required table; `mastering_display_metadata.h` joins build.rs's
`HEADERS`), and not every picture need carry them. Taken from whichever picture is decoded first, a frame would depend on where reading began. So: a stream whose TAG is PQ has its first picture decoded at `Source::open`, on the opening thread,
and its peak stands for the source's life; `PrebuiltIndex` hands it to the file's other decoders (one extra keyframe per open, HDR10 files only). A stream not tagged PQ whose pictures are: 1000. MaxCLL of 0 is "not
stated". The peak is clamped to 203..10000 and rounded to 1 cd/m²; 203 and below map without a roll-off. HLG has no peak: its nominal 1000.

## D3. The integer pipeline (`kt-ffmpeg/src/hdr.rs`, pure; one definition)

In: a picture. Out: RGBA8 at the decode size. `Rgba::convert` takes it when the path is not `Sdr`; its swscale context then goes to `yuv444p16le` (`yuva444p16le` with alpha, `rgba64le` from RGB) with the same flags, so scaling and chroma are
placed as for every other source; the context's key gains the target. Then, per pixel:

1. **Matrix** (YUV only), Q15, rounded, clamped to 0..65535. `Y0` = 4096 limited, 0 full; `C0` = 32768.
   `R' = (ky·(Y−Y0) + rv·(Cr−C0)) >> 15`, `G' = (ky·(Y−Y0) − gu·(Cb−C0) − gv·(Cr−C0)) >> 15`, `B' = (ky·(Y−Y0) + bu·(Cb−C0)) >> 15`, each with +16384:

   | | ky | rv | gu | gv | bu |
   |---|---|---|---|---|---|
   | BT.2020 limited | 38304 | 55222 | 6162 | 21396 | 70456 |
   | BT.2020 full | 32768 | 48320 | 5392 | 18722 | 61650 |
   | BT.709 limited | 38304 | 58974 | 7015 | 17531 | 69490 |
   | BT.709 full | 32768 | 51603 | 6138 | 15339 | 60804 |
   | BT.601 limited | 38304 | 52503 | 12887 | 26743 | 66359 |
   | BT.601 full | 32768 | 45941 | 11277 | 23401 | 58065 |

   FCC and SMPTE 240 by the same formula `round(c · 65535/56064 · 32768)` (luma) and `· 65535/57344` (chroma); a test derives the table from (Kr, Kb).
2. **To linear**, `u32` Q24, 1.0 = 203 cd/m² (PQ, HLG) or SDR white (Wide): a table of 4097 entries at `i·16`, index `v >> 4`, linear by `v & 15`.
   - PQ: ST 2084 EOTF / 203. HLG: the inverse OETF per channel, then the OOTF with gamma 1.2 at 1000 cd/m²: `× Ys^0.2 · 1000/203`, `Ys` = 0.2627 R + 0.6780 G + 0.0593 B.
   - Wide: `v^2.4` (Video) or the sRGB curve. The SAME curve encodes at 6, so a neutral pixel keeps its code.
3. **Tone map** (PQ with peak > 203, HLG), on `m = max(R, G, B)`: all three × `ratio(m)` Q16, so hue and saturation keep. `ratio` is the BT.2390 EETF in PQ space from the peak to 203 cd/m²: knee `KS = 1.5·maxLum − 0.5`, below it the ratio is 1.
   Above the peak: 203 (ratio `203/m`).
4. **Gamut** to BT.709 in linear, Q14 signed, rounded; rows sum to 16384, so white and greys stay:
   BT.2020: `[27206, −9628, −1194; −2041, 18562, −137; −297, −1648, 18329]`; P3-D65: `[20069, −3685, 0; −689, 17073, 0; −322, −1288, 17994]`.
5. **Into range without a hue turn**: `Y = 0.2126 R + 0.7152 G + 0.0722 B` (Q14). Where a channel is below 0 or above 1.0, all three move toward `Y` by the least `t` (Q16, rounded so the channel lands inside) that brings every channel in;
   `Y` itself is clamped to 0..1 first. Integer division only for those pixels.
6. **Encode** to 8 bits: `x^(1/2.4)` (or sRGB for a `Srgb` Wide), Q8, `(v + 128) >> 8`; 1.0 and above is 255. Alpha from the 16-bit plane `(a + 128) / 257`, else 255.

Functions of a linear value (`Ys^0.2`, `ratio`, the encode) are read from an exponent-and-mantissa table: 128 steps an octave, linear between (`x < 128` indexes directly): a plain table over Q24 has no steps left for the darks. Products that pass 32 bits are `i64`.

**What it gives** (cd/m² → code; the owner's eye check and t3803's numbers):

| | 1 | 10 | 26 | 50 | 100 | 203 | 400 | 1000 |
|---|---|---|---|---|---|---|---|---|
| PQ, peak 1000 (knee 88) | 28 | 73 | 108 | 142 | 189 | 230 | 250 | 255 |
| PQ, peak 4000 (knee 40) | 28 | 73 | 108 | 142 | 178 | 210 | 233 | 250 |
| HLG (signal 0.38 / 0.5 / 0.75 / 1.0 = 26 / 51 / 203 / 1000) | | | 109 | 143 | | 230 | | 255 |

Diffuse white (203) is NOT code 255: a map that puts it there has no codes left for highlights, and they clip. This is mpv's `bt.2390` with its default target. An earlier revision's text said both at once ("203 at the SDR
reference white, highlights rolled off below the SDR peak"), which no curve does.

The tables are built once per (path, peak) in the process from `kt_plugin_api::math` (frozen, t1066), about 60 KB each, at most 8 kept. kt-ffmpeg gains the dependency on kt-plugin-api (kt-time and serde, no FFmpeg).

**Threads**: the pipeline runs inside the converter that has the picture; a span's converters are already parallel. No rows are shared between threads: the bytes cannot depend on their number.

## D4. Why at the decode and not in the plan

F-CLR-008 wanted conversions in the plan. The plan reads the burst cache, which holds RGBA8 (invariant 4): a tone map after that quantization bands. A 16-bit cache for HDR sources only would double
their bytes and touch every reader: that is the 16-bit path (D8–D15), chosen per project.

## D5. Names: only the mapped sources move

- `FfmpegSource` keeps `file_ident` (what `cursor_for` compares) and answers `identity()` = the same mixed with `HDR_MAP_VERSION` (1) when its path is not `Sdr`. The burst cache, stream positions, the moment key, the disk tier's key and the render index
  of an HDR source are new names; an SDR project keeps its prerender.
- `Made.pixels` becomes its own constant (1): it is the conversion of EVERY file, and it does not move here.
- `THUMB_PIXELS_VERSION` (1 → 2) and `STRIP_ROW_VERSION` (3 → 4) move for all: a thumbnail is keyed by the file's content and read before the file is opened, so it cannot ask the path. They are drawn again once.
- `persistent-cache`: a copy of a mapped source is written from mapped pictures, tagged BT.709 SDR, and its name says so (`copy-<w>x<h>.m1.mkv`): a copy an earlier build made from unmapped ones is no copy of this source any more.

## D6. Facts and telemetry

- `MediaSource::colour(&self) -> Option<ColourFacts>` (default `None`; `FfmpegSource` answers; a field of `MediaInfo` would touch its 39 makers): transfer, primaries, matrix, range, peak, path.
- `kt-cli probe` prints it per file: "HDR PQ → SDR, peak 1000 (MaxCLL)", "HDR HLG → SDR", "wide BT.2020 → 709", "SDR", "SDR (transfer 8 read, not mapped)".
- The trace: a record at a source's open with the path and the peak (`Ev::Decoded` keeps its fields: one picture's record need not repeat a fact of the file).
- The media bin's file facts line says the same in words (i18n ×5): "HDR (HLG), shown in SDR".

## D7. Class walk and the cost

| Place | After |
|---|---|
| `Rgba::convert`, single and span threads, every size of a moment | D3 |
| device decode | the same planes, the same `convert`: t3807 |
| the disk tier's segments, `persistent-cache` copies | BT.709 SDR by our own tags: `Sdr` |
| copies' making (`Source::next_video`) | through `to_rgba`: mapped (D5) |
| thumbnails, filmstrip, bin scrub, reel strips, posters | the same decode; versions moved |
| stills: AVIF/HEIF in PQ or HLG, PNG with P3 (`cHRM`/`cICP`) | by the same table (their tags are the picture's); JPEG and untagged: today's bytes. An ICC profile is a follow-up (F-CLR-008) |
| alpha (ProRes 4444 HLG) | kept (D3.6) |
| generators, titles, the export's RGB→YUV, scopes, LUTs | unchanged: the working space is BT.709 SDR |
| Datamosh, Tape, ISP over an HDR file | read the mapped picture |
| audio, peak pyramid | no picture |
| `magic-wand-mask`, `chroma-key-upgrade` (a picked colour) | a colour picked from an HDR clip before this is another colour after: the key's old number is the old picture's. Said in What's new |
| a project with an HDR clip graded by hand to fight the grey | looks different: said in What's new; a per-file override is a follow-up (below) |
| OTIO, `.kt` | untouched |

**The override** ("treat this file as SDR / PQ / HLG") is not here: a wrong tag is seen in the facts (D6), and the override needs a field on the media (MODEL, a format by content). A row for after `shared-events`.

**Cost**: measured on release before the merge (task 2.5): a 4K HDR10 frame's `convert` beside its decode, one thread, `kt-cli probe … --time`. The budget is stated there, not here; if the map costs more than the decode, the
tables' layout is looked at before anything is merged.

## D8. The precision setting

- `ProjectSettings.precision: Precision { Rgba8, Rgba16F }` with default `Rgba8`. `Sequence.precision:
  Option<Precision>`, where `None` means the project's.
- A plan is built for one precision: the frame's own (the root's, or the reel's for its sub-plan). The
  precision is part of the plan key and of the burst cache key (invariant 4: the bars ask for the
  precision the plan reads).
- Changing it is an edit: undo returns it; the player rebuilds as for any settings edit; caches of the
  other precision stay until evicted.
- **The file:** the field is written only when it is `Rgba16F`. Such a file is the next free project-file format (taken at merge) by
  content (`version_for`; an old reader refuses it, naming the version, as the format rules require). An
  8-bit project is byte-identical to before.
- **The OTIO door** writes it into `metadata["kt"]` and adds a row in "What OTIO export loses" (other
  editors read an 8-bit timeline).

## D9. The working space of the 16-bit path (the owner's Q1 a)

- Scene-linear light in BT.2020 primaries, `Rgba16Float`, premultiplied alpha, with 1.0 = 203 cd/m² (BT.2408
  HDR reference white).
- An SDR source is converted to linear through BT.1886 and its own primaries: SDR white becomes 1.0.
- A PQ source maps its absolute luminance by `L / 203`. A 1000 cd/m² highlight becomes 4.93; f16 holds up
  to 65504 and keeps 10 bits of mantissa, more than a 10-bit source's steps at every level.
- An HLG source is converted with the BT.2100 OOTF at the nominal 1000 cd/m², then scaled likewise.

## D10. The decode

The D3 pipeline runs on every picture of a 16-bit plan up to its gamut step, with no tone map (gamut to BT.2020 instead of
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

## D12. Effects: a declared precision and the 8-bit island (the owner's Q2 a)

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

## D13. The display: HDR on an HDR monitor at once, SDR elsewhere (the owner's Q3 b)

- **The surface is chosen at start, and again when the window moves to another monitor.** The display asks
  wgpu for the surface's formats and colour spaces. An HDR surface is taken where one is offered:
  - **Windows:** `Rgba16Float` with the extended linear sRGB colour space (scRGB) on DX12 or Vulkan, when
    Windows HDR is on for that monitor;
  - **macOS:** `Rgba16Float` with the extended linear colour space (EDR);
  - **Linux:** a Vulkan surface with an HDR10 (ST 2084) or extended linear colour space where the
    compositor exposes it (Wayland's colour-management protocol). X11 and compositors without it stay SDR.

  eframe takes the chosen format through its existing-device setup, the same path as
  `stream-picture-stays` D2.
- **On an HDR surface** the display pass writes the linear composite without a tone map:
  - for scRGB and EDR, scaled by 203/80, since in those spaces 1.0 is 80 cd/m²;
  - for HDR10, through the PQ inverse EOTF;
  - highlights above the monitor's reported peak roll off by BT.2390 to that peak (the monitor's
    HDR metadata, when the system reports it; else 1000 cd/m²).

  The interface itself (egui) is drawn at SDR white on that surface, so it does not glare.
- **On an SDR surface** (every other case, and GL) the display pass tone-maps by BT.2390 from the frame's
  peak to the monitor's SDR white (the same curve as D3 step 3, in float) and writes `Rgba8Unorm` as
  today.
- **What the person sees:** the display's facts (Diagnostics, the trace's machine facts) name the surface
  and why: "HDR (scRGB, peak 600 cd/m²)" or "SDR (tone-mapped): the compositor gives no HDR surface".
- The scopes (change `scopes`) read the f16 composite, so they show nits above 100.

## D14. Export from a 16-bit project (the owner's Q4 a)

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
| decode | D1–D4: matrix, and HDR/WIDE mapped to SDR 8-bit | D10: linear BT.2020 f16 |
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
| project file | unchanged | the next free format when 16-bit |
| OTIO | unchanged | `metadata["kt"]`, a row in "What OTIO export loses" |

## D16. Stages and tests

1. Tags: the transfer on every major, the peak from the first picture whichever frame is asked first (t3800); the table of D1 (t3801).
2. The pipeline: SDR does not move (t3802); PQ, HLG and Wide frames by 1 and 4 converter threads (t3803); steps (t3804); hue and neutrals (t3805, t3806); the constants derived (t4142).
3. A device decode equals the processor's (t3807), where `device-decode-sources` is in.
4. Names (t4143), facts and trace (t3810, t3811). t3808 and t3809 stay free (the export default was built as t3603).

6. The 16-bit path (the owner answered Q1–Q4, kkt-26.11.20b B4):
   - the precision setting, the file's next free format, OTIO (t3877–t3880);
   - the decode to linear f16, exact on every processor (t3881–t3883);
   - the pool by format, the core float shaders, `FrameTol` float classes (t3884–t3887);
   - the effect's precision, the island and its mark, the colour family's float kernels (t3888–t3891);
   - the display: the HDR surface where offered, the tone map elsewhere (t3892);
   - the export SDR / HDR10 / HLG with their tags and mastering metadata (t3893–t3895);
   - the reels' boundaries and the prerender's Main10 segments (t3896, t3897);
   - the settings' control and the chain's mark (t3898); the idle check on a 16-bit heavy showcase (t3899).

## Risks (the 16-bit path)

- **Memory doubles** for every picture of a 16-bit project. The ledger's budgets stay in bytes, so fewer
  frames fit; the trace shows it.
- **Effects in islands** lose highlights. The marks show where, and the float kernels follow by family.
- **HDR surfaces** are uneven across systems. The owner asked for HDR at once where the system can (Q3 b);
  the tone-mapped SDR display is the path that always works, and the facts say which one runs.
