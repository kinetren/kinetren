## ADDED Requirements

### Requirement: An HDR or wide-gamut picture is mapped to the SDR working space at decode
The decode SHALL read a picture's transfer (`AVColorTransfer`) with its matrix, range and primaries; a
tag the picture lacks is taken from the stream. Where the stream carries them, it SHALL also read the
mastering display's peak and MaxCLL.

A picture SHALL be mapped to the working space (BT.709 primaries, BT.709 SDR encoding, 8-bit RGBA) by one
integer pipeline:
- a PQ or HLG transfer takes the HDR path: scale and upsample to 16-bit 4:4:4, matrix, EOTF, gamut to
  BT.709 in linear light, tone map, out-of-gamut colours toward their luminance, SDR encoding;
- a BT.2020 or P3-D65 picture on an SDR curve takes the same pipeline without the tone map;
- every other picture keeps the conversion of "A source's YUV becomes RGB with its own matrix and range"
  byte for byte.

The tone map SHALL place the HDR reference white (203 cd/m², BT.2408) at the SDR reference white and roll
highlights off by the BT.2390 curve from the source's peak (the mastering levels, else 1000 cd/m²). The
pipeline's tables SHALL be built from the frozen math (`kt_plugin_api::math`), so its bytes are the same
on every processor. A device decode SHALL run the same integer arithmetic and give the same bytes. The
path a source takes SHALL be in its facts, in `kt-cli probe` and in the trace's decode record. Disk caches
of decoded pixels SHALL move their versions with the change.

Why: found-defects 724 — a BT.2020 HLG/PQ source (an iPhone's default) was converted with the matrix
alone and drawn as SDR BT.709: grey and washed out; and its 10 bits were cut to 8 before any mapping could
see them, so the map belongs at the decode, not in the plan (hdr-sources-to-sdr D1–D3).

#### Scenario: The transfer and the mastering peak are read
- **WHEN** fixtures tagged PQ with mastering metadata, HLG, BT.2020 SDR, P3 SDR and untagged are opened on every FFmpeg major
- **THEN** each reports its transfer and primaries, the PQ one its peak, and the numbers equal libavutil's on each major
- **TEST** t3800 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: The path is resolved by the tags
- **WHEN** every transfer × primaries pair of the table in design D1 is resolved, with an unknown transfer among them
- **THEN** PQ and HLG take the HDR path, wide primaries on an SDR curve the WIDE path, the rest today's path, and the unknown one is reported as read
- **TEST** t3801 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: An SDR source does not move a byte
- **WHEN** the golden projects and an SDR BT.709 and BT.601 fixture are decoded and rendered
- **THEN** every frame and GOLDEN_VIDEO equal the former bytes
- **TEST** t3802 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: PQ and HLG map to their stored frames on every processor
- **WHEN** the PQ and HLG fixtures are decoded at two sizes, in a scalar and a row-parallel run
- **THEN** the pictures equal their stored frames byte for byte, and the HLG one's mid-grey and the PQ one's 203 cd/m² patch land at the SDR reference level within 2 codes
- **TEST** t3803 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A 10-bit gradient keeps its steps
- **WHEN** a PQ 10-bit ramp from 0 to 1000 cd/m² is decoded
- **THEN** the output holds more than 200 distinct levels and never steps down
- **TEST** t3804 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A colour outside BT.709 keeps its hue
- **WHEN** a saturated BT.2020 green and a P3 red are decoded
- **THEN** both are inside BT.709 with no negative channel, their hue within 3° of the source's
- **TEST** t3805 (`src/libs/kt-ffmpeg/src/colour.rs`), t3806 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The device maps as the processor does
- **WHEN** the PQ and HLG fixtures are decoded by the device decode and by the processor
- **THEN** the pictures are equal byte for byte
- **TEST** t3807 (`src/libs/kt-gpu/tests/device_decode.rs`)

#### Scenario: The path is named where a complaint is read
- **WHEN** `kt-cli probe` runs over a project with an HLG clip, and its trace is read
- **THEN** the probe prints "HDR HLG → SDR, peak 1000" for the clip, and the trace's decode record of its frames names the path
- **TEST** t3810 (`src/apps/kt-cli/tests/cli.rs`), t3811 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)
