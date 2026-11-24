# Design: hdr-sources-to-sdr

> **D1–D7, the 8-bit path (the default, "быстрый i8"), are IMPLEMENTATION-READY** (spec-engine, revision 3, 2026-11-24, orch `229c694b`,
> found-defects 724). D8–D15, the 16-bit path ("тракт f16", kkt-26.11.20 A25), are IMPLEMENTATION-READY as six sessions
> (revision 4, 2026-11-25, orch `fc60a5d9`); the stage goes after the GPU, STREAM and COLOR-FX rows that run, so that nobody writes a shader twice.

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

- `ProjectSettings.precision: Precision { Eight, Deep }`, default `Eight`; `Sequence.precision: Option<Precision>`, `None` — the project's. Not a property of an event or a track. `PROJECT_FILE_VERSION_DEEP`, by content; an 8-bit project is byte-identical. OTIO: `metadata["kt"]` and a row in "What OTIO export loses".
- **A plan has one precision**: the root's or the reel's. `FramePlan.deep: bool`; a reel's sub-plan its own; a track stage's, a front's, a member's (real-3d-groups) its parent's.
- **The order**: `Quality` gains `deep: bool` (311 literals: `Quality::new(w, h)` and a mechanical commit of its own first, no behaviour). It travels with every request, burst key, coverage question and `layers_at` row, so "the order is the read" holds with no second rule; the bars ask with the plan's.
- **Keys**: `put_plan` hashes `deep` only when set; leaves carry the size AND the form (`Leaf::frame(ident, w, h, deep)`), so an 8-bit project's keys do not move. `Made.pixels`' deep twin is `HDR_MAP_VERSION` + 1 in the deep leaves.
- Changing it is an edit (undo); the other precision's frames stay until evicted.

## D9. The working space (the owner's Q1 a)

Scene-linear light, BT.2020 primaries, `Rgba16Float`, **straight alpha** as the 8-bit path's (the passes that premultiply do it where they do today; an earlier text said premultiplied: every twin would then differ from its 8-bit text), 1.0 = 203 cd/m². SDR white of any SDR source, generator or title is 1.0; a 1000 cd/m² highlight 4.93. f16 holds 65504 and a relative step
of 2⁻¹¹, finer than a 10-bit source's everywhere.

**The peak of the project is 1000 cd/m²** (4.93), a constant of this change: the tone map of the display, of the islands, of an SDR export and of a boundary is D3 step 3's BT.2390 curve from 1000 to 203, one function, in integers on the processor and in the same table on the device. Revision 3's "from the frame's peak" would make one moment's picture depend on its brightest pixel, and its neighbours'
brightness pump. HDR10's mastering display says 1000. A row for a project peak: after the owner has seen it.

## D10. The decode

`hdr.rs` (D3) with `Out::Deep`: every picture of a deep request, SDR ones too, goes to `yuv444p16`/`rgba64` at the decode size, then matrix, to linear Q24, gamut to BT.2020 (Q14, from BT.709: `[10279, 5395, 710; 1132, 15066, 186; 269, 1442, 14673]`, from P3-D65 derived and held by t4142), no tone map, then the f16 bits by an exact integer conversion (leading zeros, round to
nearest even). Bytes are the same on every processor (invariant 11). `VideoFrame.form: Form { Rgba8, Rgba16F }` (79 literals: `VideoFrame::rgba8(..)`, a mechanical commit); `rgba` stays the byte box, `w·h·8` long. `blend_frames` (frame blending) gets its f16 twin, in f32, rounding once.

## D11. The GPU executor

- **Two texture forms**: `Rgba8Uint` (today) and `Rgba16Float` (`T0..T2: texture_2d<f32>`, unfilterable binding: `textureLoad` only, as the 8-bit passes do; renderable everywhere wgpu runs). `pool.rs` `FORMAT` becomes a property of the acquire; every byte count goes through `bytes_of(format)`.
- **Two sets of core pipelines**, `Pipe` × form, compiled when a plan of that form is asked (an 8-bit project compiles none): `composite`, `layer_mix`, `premul`, `unpremul`, `resample`, `mirror`, `mask`, `matte`, `grid_tile`, `geom_fused`, `display`, `yuv`, `held_planes`. The texts are twins beside the 8-bit ones (`composite_deep.wgsl`…): the 8-bit texts are integer and byte-frozen, and one text
  with a form switch would move their bytes. Integer arithmetic becomes the same formula in f32 without rounding; blend modes as `blend_ch`, with inputs clamped to 0..1 only where the formula has no meaning outside (screen, overlay, difference: the part above 1 passes over).
- `FrameTol::DEEP`: |Δ| ≤ 2⁻⁹ relative (or 2⁻¹² absolute below 2⁻³), ×2 of what the software adapter and a card differ by, set by the executor; two runs on one device are bit-equal.
- A transition's WGSL reads `samp` in 0..255 floats: the deep composite hands it encoded extended values (D12) and decodes what it returns, so all 36 kinds work at once; the crossfade is done in linear light (its 8-bit twin is integer; the look differs there as light does, said in the guide).
- `fs_y`/`fs_uv` (export-into-surface) take the deep twin of the conversion like `fs`.

## D12. Effects (the owner's Q2 a)

**The encoded extended form.** `enc(x) = sign(x)·|x|^(1/2.4)·255` in BT.2020, with no ceiling: for SDR content the numbers an 8-bit kernel reads today, beyond 255 for highlights.
- **Extended taps.** A point-wise tap (`tap_<id>`) returns an unclamped `vec4<f32>` already; only the host clamps after it. A tap declared `Decl.extended` runs in a deep chain on `enc(x)` with no round and no clamp, and is decoded once at the end of its run: no second text. Declared in this change after being read one by one: `color_grade`, `vignette`, the LayerMix's side, gain-like taps. Not: Posterize, Duotone,
  Greenscreen (tables and thresholds over 0..255).
- **Geometry, CamMotion, Pan3d, warps** resample: their fused pass has its deep twin (D11) and they need no declaration.
- **The core's mip blur** (`Node::Resample` graphs of plugins with no shader of their own) runs deep.
- **Everything else is an island**: barrier kernels with WGSL, thresholds, streams.
  1. in: tone map to 203, gamut to BT.709, encode, `Rgba8Uint` (the SDR rendition, one pass `to_sdr`);
  2. the 8-bit passes, unchanged;
  3. out (`from_sdr`): decode, gamut to BT.2020; where no channel moved by more than one code from the island's own input at that pixel, the input's linear value comes back whole (highlights kept where the effect did nothing); elsewhere the decoded value times the tone map's inverse at its level (so an effect that leaves a bright area bright keeps its brightness up to the peak).
  Neighbouring islands share one `to_sdr`/`from_sdr` (`kt_fx::plan` gains `PlanOp::Island { open | close }` around runs of non-deep ops; the plan of an 8-bit chain is untouched).
- A chain row of an island shows "8-bit" and its hint. `Manifest` has no `precision` list: the planner knows the class, `Decl.extended` is the only fact a plugin gives; a wasm or OFX plugin is an island (OFX may say float: openfx-host D13).
- **Stream stages** read the SDR rendition of their input (the front runs deep, the last pass is `to_sdr`), their picture enters by `from_sdr`'s decode.

## D13. The display (the owner's Q3 b)

1. **Always**: the deep twin of `display.wgsl` tone-maps into `Rgba8Unorm` (session 5a).
2. **An HDR surface** (session 5b). wgpu 30 offers what is needed: `SurfaceCapabilities` lists per format its `color_spaces`, `SurfaceConfiguration.color_space` takes `ExtendedSrgbLinear` (scRGB, EDR: `Rgba16Float`) or `Bt2100Pq`. egui-wgpu 0.36.1 does not: it picks `Rgba8Unorm` first and the colour space `Auto`, and its renderer has no linear, scaled output. So egui-wgpu is vendored
   (`src/vendor/egui-wgpu`, `[patch.crates-io]`) with three small changes, each behind a config field: the surface's (format, colour space) chosen by a callback; an output mode `Linear { sdr_white }` that decodes the interface's gamma colours and scales them; the reconfiguration on demand. The patch is kept by a script that re-applies it on an upgrade and a guard (dist-check) that the vendored
   version is eframe's. Rejected: our own painter (the whole of eframe's window loop); a second surface over the preview (no such thing on Wayland and X11 without a subsurface eframe does not give).
3. The preview's picture on an HDR surface is drawn by a paint callback with the deep texture, linear, × 203/80 for scRGB/EDR, PQ-encoded for `Bt2100Pq`; above the monitor's peak it rolls off by BT.2390 (the peak: the system's number where wgpu or the window gives one, else 1000). The interface is at the system's SDR white (203 when unknown).
4. When: at the start and when the window moves to another monitor or the surface is remade (gpu-errors-survive's path); one surface form for the whole window. A project with no deep composition keeps `Rgba8Unorm` even on an HDR monitor: the interface must not change for a person who asked nothing.
5. Where it is SDR and why is a line of the machine facts and Diagnostics: "HDR (scRGB, peak 600)", "SDR: the window system offers no HDR surface (X11)". What Wayland compositors offer is the owner's check (a C row): nothing of it can be seen on the stand.
6. Every reader of shown bytes (`read_display`, screenshots, the poster, `kt-cli preview`) reads the SDR rendition.

## D14. Export (the owner's Q4 a)

- **SDR**: tone map, BT.709, then the 8- or 10-bit conversion as today.
- **HDR10**: PQ of `x·203` cd/m², clipped at 10000; **HLG**: the inverse OOTF (γ 1.2 at 1000) and OETF. Both BT.2020 non-constant luminance, limited range, 10-bit `yuv420p10`/P010/`yuv422p10`, chroma left. `YuvLayout` gains `transfer: Sdr | Pq | Hlg` and `YuvMatrix::Bt2020`; the pass reads f16 and encodes by the curves' f32 formulas from `kt_plugin_api::math`'s coefficients, rounding once; the processor's
  reference is the same f32 text, held to ±1 code of 1023 (one device's two runs are equal).
- **Metadata**: mastering display (BT.2020, D65, 0.005–1000) and content light. MaxCLL/MaxFALL are measured while composing (a reduction of the linear picture, 2×2 steps on the device, read with the planes): they are known at the END. MKV and MP4 write them as stream side data in the header: so the header's numbers are 0 ("not stated") at first and rewritten at `finish` where the container can (MP4's `clli` box and MKV's
  element are of fixed size: our muxer rewrites them in the closed file by the offsets it found when writing the header, a plain seek, no second pass; `+faststart` moves the header: the rewrite is done after it). A container where it cannot be found keeps 0 and the report says so. The x265/NVENC SEI gets the mastering display at the opening, MaxCLL 0.
- **Offered** with: libx265, libsvtav1, libaom-av1, libvpx-vp9 (profile 2), prores_ks, hevc_nvenc, av1_nvenc (the rows with `pix10`), MKV and MP4/MOV; not libx264 High10 (players refuse PQ there). VAAPI and QSV rows have no `pix10` today: with one they join.
- `cached-final-render`: the fingerprint gains the transfer; `loudness-norms`, audio: nothing.

## D15. Boundaries and the class walk

- **A reel of another precision**: its sub-plan is composed in its own form, then `to_sdr` (deep reel in an 8-bit parent) or `from_sdr`'s decode (8-bit reel in a deep parent: SDR white 1.0), one pass before the instance's chain. The reel's own kept frames are of its own form.
- **The prerender** of a deep project: `SegmentCodec` gains `deep: bool`; HEVC Main10 PQ P010 by the 10-bit hardware rows (NVENC today); none opens → nothing is written, and the store says "no 10-bit hardware encoder". The restored picture is decoded through D10's path (PQ, peak 10000, no map): within the codec's loss. `persistent-cache` copies of sources: an HDR source's copy for a DEEP plan is the original (a copy is SDR by D5):
  the copy is not asked, said on the card ("copies are 8-bit: this composition plays the original").
- **Generators and titles**: drawn in 8 bits and decoded at upload (`from_sdr` without the island rule); the title's edges are SDR by nature. A deep title kernel is not here (an earlier text promised it: it has nothing above white to keep).

| Place | Deep path |
|---|---|
| burst cache, coverage, pins, the ledger | by bytes, 8 a pixel; `Tenant` budgets as they are: half the frames |
| `ReadbackCache`, present memo, kept composites, upload cache, transients | `bytes_of(format)` |
| thumbnails, filmstrips, strips, posters, effect cells, the bin | `Eight` always (their orders ask `deep: false`) |
| masks, mattes, shape coverage | coverage textures stay 8-bit `Rgba8Uint`; the deep passes read them as uint (T2) |
| scopes | read the deep composite: nits axis (the `scopes` change's own row) |
| chroma key, magic wand | their plugins are islands; a picked colour is the SDR rendition's |
| simulated sprites, real-3d-groups' pass | sprites: `from_sdr` colours in the resolve; the mesh pass: `Rgba16Float` MSAA target (renderable at 4 samples on every native adapter; blendable: asked at the device's opening, absent → the group is an island) |
| track-stream-stage, Datamosh, Tape, ISP | islands |
| export: pipelined, serial, surfaces, cached, draft | D14; the draft reads the tier as it is |
| render-ahead, the arbiter, pass costs | per (pipe, form) slots |
| `picture-check`, `kt-cli` picture hashes | of the SDR rendition, as a person sees |
| export-host-copy's readback | planes as today |
| a machine whose adapter lacks blendable/renderable `Rgba16Float` | none of wgpu's native ones; GL ES 3.0 without the extension: the project's deep precision is refused with its reason at opening the setting, an opened deep project shows 8-bit and says so |

## D16. Stages and tests

1. Tags: the transfer on every major, the peak from the first picture whichever frame is asked first (t3800); the table of D1 (t3801).
2. The pipeline: SDR does not move (t3802); PQ, HLG and Wide frames by 1 and 4 converter threads (t3803); steps (t3804); hue and neutrals (t3805, t3806); the constants derived (t4142).
3. A device decode equals the processor's (t3807), where `device-decode-sources` is in.
4. Names (t4143), facts and trace (t3810, t3811). t3808 and t3809 stay free (the export default was built as t3603).

6. The 16-bit path, six sessions, a merge after each (all behind the setting, so every merge is safe):
   - 5.1 the model, the file, the two mechanical commits (`Quality::new`, `VideoFrame::rgba8`), keys (t3877–t3880, t3883);
   - 5.2 the decode (t3881, t3882);
   - 5.3 the executor's forms and twins, transitions, boundaries, generators (t3884–t3887, t3896);
   - 5.4 extended taps, islands, the marks, streams (t3888–t3891);
   - 5.5a the SDR display, the settings' control, the idle check (t3892 first half, t3898, t3899); 5.5b the HDR surface (t3892, t4155);
   - 5.6 the export and the prerender (t3893–t3895, t3897, t4156).

## Risks (the 16-bit path)

- **Memory doubles** for every picture of a 16-bit project. The ledger's budgets stay in bytes, so fewer
  frames fit; the trace shows it.
- **Effects in islands** lose highlights. The marks show where, and the float kernels follow by family.
- **HDR surfaces** are uneven across systems. The owner asked for HDR at once where the system can (Q3 b);
  the tone-mapped SDR display is the path that always works, and the facts say which one runs.
