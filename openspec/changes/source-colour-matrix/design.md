# Design: source-colour-matrix

Found-defects row 697, found by `render-settings` (stage 17). Its D6 made the export tag the matrix
and range it converts with; the decode still converted every source with swscale's default.

## D0. The mechanism

`sws_getContext` without `sws_setColorspaceDetails` converts YUV as BT.601, limited range (full only
for a `yuvj*` format, which swscale recognises by itself). Nothing read the stream's tags. A BT.709
file — most HD from cameras and phones, and every untagged HD file by the players' convention — was
shown with its reds and greens 12–14 levels off: the fixture's #C03020 came back as (180, 33, 33)
instead of (192, 47, 31). The preview, the thumbs, the reel previews and the export all start from
this one conversion, so all of them carried the shift, and the export (now tagged BT.601) kept it.

## D1. The rule (`kt_ffmpeg::colour`, pure)

- A picture's own tags (`AVFrame.colorspace/color_range/color_primaries`) win; a field it leaves
  unspecified is taken from the stream's codec parameters (the container's `colr`, the SPS VUI).
- Matrix tags swscale can convert: BT.709 (1), FCC (4), BT.470BG/SMPTE 170M (5, 6 → BT.601),
  SMPTE 240M (7), BT.2020 NCL/CL (9, 10 → its NCL matrix; constant luminance has no swscale matrix).
  Anything else — unspecified, reserved, RGB on a YUV picture, YCgCo, ICtCp — falls to the rule.
- The rule is mpv's and the browsers': ≥ 1280 wide or > 576 lines is HD → BT.709, else BT.601. A
  portrait phone video (720×1280) is HD.
- Range: the tag (MPEG → limited, JPEG → full); unspecified → full only for a `yuvj*` format.
- The numbers are H.273's and fixed in libavutil; t2891 compares them with every major's headers, so
  a module can never disagree silently.

Rejected: reading the tags only once at open. Frames carry them (a stream can switch matrices in an
SPS change; a mov `colr` may disagree with the VUI, and libavcodec gives the bitstream precedence on
the frame), and the per-picture read costs nothing.

## D2. The conversion

`Rgba::convert` resolves the picture's colour before choosing the context; `(matrix, range)` is part
of `SwsKey`, so a context made for one colour is never reused for another. A new context gets
`sws_setColorspaceDetails(src = getCoefficients(matrix), srcRange, dst = BT.601 table, dstRange 0,
brightness 0, contrast and saturation 1.0)`; the RGB side's table and range are swscale's own
(unused for an RGB destination, left as they were).

Only YUV pictures are touched: a format with three components that is neither RGB nor a palette.
Grey, RGB (PNG) and PAL8 keep swscale's conversion.

## D3. Determinism (invariant 11)

The conversion is a function of the picture's bytes and tags — no state, no clock. A picture whose
colour resolves to swscale's own default (BT.601, the format's range) does not call
`sws_setColorspaceDetails` at all: an untagged SD file and a JPEG still convert exactly as in every
former build. Every fixture of the golden and conformance tests is SD and untagged (or a JPEG), so
their stored bytes do not move (t2893 and the whole suite). An HD or tagged source changes its bytes
once, deliberately, and then is stable run to run.

## D4. The class: every YUV↔RGB path and every keeper of decoded pixels

The owner's rule ("a problem is a class"): the mechanism is "a conversion between YUV and RGB that
does not use the matrix the material says". Walked:

| path | direction | finding |
|---|---|---|
| `source_impl.rs` `Rgba::convert` — frame read, span, multi-size span, threaded span converters | YUV → RGB | **the defect**; fixed (D2). All four reads share the one `Rgba` and its stream tags (t2892 reads three of them) |
| `rgba_conversion_overrun` (t1450's probe) | YUV → RGB | a test of the tail pad on a synthetic picture; its matrix is irrelevant to the overrun — unchanged |
| stills (`still.jpg`, PNG) | YUV / RGB → RGB | through the same decode: a JPEG is `yuvj420p` tagged BT.470BG full → swscale's default, untouched (t2893); a PNG is RGB, untouched |
| thumbnails (`kt-app thumbs.rs`) | decode → RGBA → JPEG → RGBA | the decode is fixed; the JPEG step is the `image` crate's own JFIF round trip (one matrix both ways). **Sidecar thumbs written before** carried the old colours: a new additive table `thumb_pixels` (version 1); thumbs without the current version are not read, and the first new write drops the media's older thumbs so a filmstrip never mixes the two (t2894). The same hole existed for the t719 `thumb_meta` transition (an older thumb at another moment was read once a new size row appeared); the drop closes it too |
| reel previews (`reel_preview.rs`, `reel_frame` in the sidecar) | rendered from decoded sources | **disk tier keyed by edit and media, not by decoder** — `STRIP_ROW_VERSION` 2 → 3, every strip renders again once |
| welcome-window posters | rendered RGBA → PNG | written when the project is saved; an older poster is replaced at the next save — no key to move, accepted |
| kept composites, kept mix, burst/frame caches | — | memory only, built by this process |
| GPU upload (`kt-gpu`) | RGBA textures | no YUV upload exists; `display.wgsl` draws RGBA. Nothing to do |
| export (`muxer_impl.rs` `VideoConverter`) | RGB → YUV | converts with the chosen matrix/range and tags the same (row 696, t2806); the VAAPI path converts to NV12 through the same converter. Consistent. Its default stays BT.601 limited: moving it to BT.709 changes every export's bytes — a follow-up for render-settings, not this change |
| generators, titles (`kt-text`) | RGB | draw RGBA directly |
| effects `fx-video-codec` (`convert.wgsl`), `fx-video-camera` (`s09_yuv.wgsl`), `fx-video-tape` (`fin.wgsl`) | RGB → YUV → RGB inside the effect | simulations' own round trips with one matrix both ways — not a source conversion |
| `kt-cli` probe/export tests (`cross_render`, `oc_render`) | ffprobe expectations | strings of the export's tags; unchanged |

Not covered here (filed, found-defects row 724): primaries and transfer are read but not converted —
a BT.2020 / PQ / HLG source gets the BT.2020 matrix only, no gamut mapping or tone mapping, so wide-
gamut and HDR footage still looks washed out against a player that tone-maps.

## D5. Cost

One `av_pix_fmt_desc_get` and a few compares per converted picture; a context is still made once per
(size, format, colour). Idle cost is unchanged: nothing runs at rest that did not before.
