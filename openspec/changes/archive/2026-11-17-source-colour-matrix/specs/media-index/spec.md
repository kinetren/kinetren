## ADDED Requirements

### Requirement: A source's YUV becomes RGB with its own matrix and range
The decode SHALL convert a YUV picture to RGBA with the matrix and range the picture's tags name, a
tag the picture lacks taken from the stream's codec parameters. A matrix tag swscale cannot convert
(unspecified, reserved, RGB, YCgCo, ICtCp) SHALL fall to the rule players use: a picture 1280 or
more wide or taller than 576 lines is BT.709, a smaller one BT.601; an unspecified range SHALL be
full only for a `yuvj*` pixel format. A picture whose resolved matrix and range are swscale's own
default (BT.601, the format's range) SHALL leave the conversion context untouched, so its bytes stay
those of every former build; the matrix and range SHALL be part of the context's key. RGB, grey and
palette pictures keep swscale's own conversion. Primaries are read and reported, not converted.
Disk caches holding decoded pixels SHALL carry a version that moves with the conversion: the
sidecar's thumbs (`THUMB_PIXELS_VERSION`, the older ones dropped on the first new write) and the
reel strips (`STRIP_ROW_VERSION`).
Why: found-defects 697 — every source was converted as BT.601, so a BT.709 HD file (most cameras and
phones) showed shifted colours in the preview and the export (source-colour-matrix design D1–D4).

#### Scenario: The tag wins, the rule fills in
- **WHEN** tags of every kind are resolved at HD and SD sizes, with and without a `yuvj` format, and a picture's unspecified tag meets the stream's
- **THEN** a tag beats the size, an unspecified matrix is BT.709 at HD and BT.601 at SD, the range is from the tag or the `yuvj` format, and the picture's own tag beats the stream's
- **TEST** t2890 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: The numbers are libavutil's on every major
- **WHEN** the colour numbers the rule uses are compared with the headers of FFmpeg 6, 7, 8 and 9
- **THEN** every one is equal
- **TEST** t2891 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: A BT.709 patch decodes to its colour
- **WHEN** a #C03020 patch written as BT.709 limited is decoded — tagged at 1280×720, tagged at 320×180, untagged at 1280×720 — by a frame read, at half size and in a span at two sizes
- **THEN** every centre pixel is within 3 levels of (192, 48, 32), where BT.601 gives (180, 33, 33), and the probe reports BT.709 limited, tagged or not
- **TEST** t2892 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: An untagged SD source keeps the former conversion
- **WHEN** an untagged 320×180 H.264 file and a JPEG still are probed and decoded twice
- **THEN** both resolve to BT.601 (the JPEG full range), the context is left untouched and both decodes are the same bytes
- **TEST** t2893 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Thumbs of an older pixel version are not read
- **WHEN** a sidecar holds thumbs and a size written without a pixel version, and a new thumb is then written
- **THEN** the size reads as absent until the new write, which drops every older thumb of the media and keeps the new one
- **TEST** t2894 (`src/libs/kt-media/tests/index_db.rs`)
