# Proposal: source-colour-matrix

## Why

Found-defects row 697 (found by `render-settings` in stage 17): decoding ignored a source's colour
matrix and range and converted every source with swscale's default, BT.601 limited. Most cameras and
phones write HD as BT.709, so their colours were shifted in the preview, the thumbnails, the reel
previews and the export: a #C03020 patch came back as (180, 33, 33) instead of (192, 47, 31). Row 696
already made the export tag what it writes; the decode side still guessed.

## What Changes

- The source reads the stream's `color_space`, `color_range` and `color_primaries` (the codec
  parameters) and each decoded picture's own tags, which win where they are set.
- The RGBA conversion converts with that matrix (BT.709, BT.601, FCC, SMPTE 240M, BT.2020) and range.
  An unspecified or unconvertible tag falls to the players' rule: HD (≥ 1280 wide or > 576 lines) →
  BT.709, SD → BT.601; an unspecified range is full only for a `yuvj*` format.
- A source that resolves to swscale's own default (an untagged SD file, a JPEG) leaves the context
  untouched: its pictures stay the former bytes (invariant 11).
- The matrix and range are part of the conversion context's key.
- `kt_ffmpeg::Probe` reports the tags and the resolved conversion.
- Disk caches that hold decoded pixels are invalidated once: sidecar thumbs by a new additive
  `thumb_pixels` version table, reel strips by `STRIP_ROW_VERSION` 3.
- What stays: primaries are read and reported but not converted (no gamut mapping, no PQ/HLG
  transfer); a BT.2020/HDR source gets the BT.2020 matrix only — a found-defects row of its own.

## Capabilities

### Modified Capabilities
- `media-index`: the decode converts YUV with the source's own matrix and range.

## Impact

`kt-ffmpeg` (`colour.rs`, `source_impl.rs`, `Probe`), `kt-media` (sidecar `thumb_pixels`), `kt-app`
(thumb write order, `STRIP_ROW_VERSION`), `fixtures/gen.sh` (three BT.709 patch files).
