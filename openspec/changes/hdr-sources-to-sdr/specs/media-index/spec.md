## ADDED Requirements

### Requirement: An HDR or wide-gamut picture is mapped to the SDR working space at decode
The decode SHALL read a picture's transfer (`AVColorTransferCharacteristic`) with its matrix, range and primaries; a
tag the picture lacks is taken from the stream.

In a plan of 8-bit precision (the default; capability `video-plan`, "A project and a composition choose
their picture precision") a picture SHALL be mapped to the working space (BT.709 primaries, BT.709 SDR encoding, 8-bit RGBA) by one
integer pipeline:
- a PQ or HLG transfer takes the HDR path: scale to 16-bit 4:4:4, matrix, linear light, tone map, gamut to BT.709, colours outside brought
  toward their luminance, SDR encoding;
- a BT.2020 or P3-D65 picture on an SDR curve takes the same pipeline without the tone map, decoded and encoded by one curve;
- every other picture keeps the conversion of "A source's YUV becomes RGB with its own matrix and range"
  byte for byte.

The tone map SHALL be the BT.2390 curve from the source's peak to 203 cd/m² (BT.2408's reference white, the SDR peak of the map): below its knee luminance is kept, above it highlights roll off, hue and
saturation kept. The peak of a PQ stream SHALL be its MaxCLL, else its mastering display's, else 1000 cd/m², read once when the source opens, from the stream's first picture, so a frame never depends on
where reading began; HLG's is its nominal 1000. The pipeline's tables SHALL be built from the frozen math (`kt_plugin_api::math`), so its bytes are the same
on every processor and by any number of converter threads. A picture a device decoded SHALL give the same bytes. A source whose pictures are mapped SHALL carry the map's version in its identity, and no
other source's identity SHALL move. The path a source takes SHALL be in its facts, in `kt-cli probe` and in the trace.

Why: found-defects 724 — a BT.2020 HLG/PQ source (an iPhone's default) was converted with the matrix
alone and drawn as SDR BT.709: grey and washed out; and its 10 bits were cut to 8 before any mapping could
see them, so the map belongs at the decode, not in the plan (hdr-sources-to-sdr D1–D5).

#### Scenario: The transfer and the mastering peak are read
- **WHEN** fixtures tagged PQ with MaxCLL, PQ without side data, PQ whose second keyframe alone carries another MaxCLL, HLG, BT.2020 SDR, P3 SDR and untagged are opened on every FFmpeg major present, and the PQ ones' last frame is read before the first
- **THEN** each reports its transfer and primaries; the peaks are MaxCLL, 1000 and the first picture's; a frame's bytes do not depend on the order of reads; the numbers equal libavutil's on each major's headers
- **TEST** t3800 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The path is resolved by the tags
- **WHEN** every transfer × primaries pair of the table in design D1 is resolved, with an unknown transfer among them
- **THEN** PQ and HLG take the HDR path, wide primaries on an SDR curve the WIDE path, the rest today's path, and the unknown one is reported as read
- **TEST** t3801 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: An SDR source does not move a byte
- **WHEN** the golden projects and an SDR BT.709 and BT.601 fixture are decoded and rendered
- **THEN** every frame and GOLDEN_VIDEO equal the former bytes
- **TEST** t3802 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: PQ and HLG map to their stored frames on every processor
- **WHEN** the PQ and HLG patch fixtures are decoded at two sizes, with one and with four converter threads
- **THEN** the pictures equal their stored frames byte for byte; at peak 1000 the patches of 1, 10, 26, 50, 100, 203, 400 and 1000 cd/m² read 28, 73, 108, 142, 189, 230, 250 and 255 within 2 codes, and HLG's 0.38, 0.5, 0.75 and 1.0 read 109, 143, 230 and 255
- **TEST** t3803 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A 10-bit gradient keeps its steps
- **WHEN** a PQ 10-bit ramp from 0 to 1000 cd/m² is decoded
- **THEN** the output holds more than 200 distinct levels and never steps down
- **TEST** t3804 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: A colour outside BT.709 keeps its hue
- **WHEN** a saturated BT.2020 green and a P3 red are decoded, and neutral patches of the BT.2020 SDR and P3 fixtures
- **THEN** both are inside BT.709 with no channel clipped alone, their hue within 3° of the source's; the neutrals keep the codes of today's conversion within 1
- **TEST** t3805 (`src/libs/kt-ffmpeg/src/colour.rs`), t3806 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The pipeline's constants are the standards'
- **WHEN** the matrices are derived from (Kr, Kb) and the primaries, and a million random 16-bit pixels of every path go through the kernel
- **THEN** the constants equal the table, each gamut row sums to 16384, a neutral pixel stays neutral, and no channel leaves 0..255 or is clipped without the others moving
- **TEST** t4142 (`src/libs/kt-ffmpeg/src/hdr.rs`)

#### Scenario: Only a mapped source is renamed
- **WHEN** an HLG file and an SDR file are opened by a build that maps and compared with the file's own identity, the HLG file is replaced on disk before a second decoder opens, and a copy of the HLG file made by a build that did not map lies in the cache folder
- **THEN** the SDR identity is the file's, the HLG one is not and carries the map's version; the second decoder of the replaced file is not kept; the old copy is not a copy of the source, and a new one is made
- **TEST** t4143 (`src/libs/kt-media/tests/identity.rs`)

#### Scenario: The device maps as the processor does
- **WHEN** the PQ and HLG fixtures are decoded by the device decode and by the processor
- **THEN** the pictures are equal byte for byte
- **TEST** t3807 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The path is named where a complaint is read
- **WHEN** `kt-cli probe` runs over a project with an HLG clip, and its trace is read
- **THEN** the probe prints "HDR HLG → SDR" for the clip and "SDR" for the others, and the trace's record of the source's open names the path and the peak
- **TEST** t3810 (`src/apps/kt-cli/tests/cli.rs`), t3811 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

### Requirement: In a 16-bit plan the decode gives linear light without a tone map
In a plan of 16-bit float precision the decode SHALL convert every picture by the same integer pipeline up
to linear light, as follows:
- the gamut SHALL be converted to BT.2020;
- the result SHALL be scaled so that the HDR reference white (203 cd/m², BT.2408) and an SDR source's white
  are 1.0;
- an HLG source SHALL take the BT.2100 OOTF at 1000 cd/m²;
- there SHALL be no tone map;
- the result SHALL be written as `Rgba16Float` by an exact integer conversion, straight alpha.

The frame SHALL carry its pixel format, and the frames cache key SHALL carry the precision. The bytes SHALL
be the same on every processor.
Why: kkt-26.11.20 line 33 ("если включено то тракт f16 чтобы было нормально"): the 16-bit path keeps an
HDR source's 10 bits and its highlights for the effects, the display and an HDR export
(hdr-sources-to-sdr D9, D10).

#### Scenario: An HDR source keeps its highlights and steps
- **WHEN** the PQ fixture with a 1000 cd/m² patch and the 10-bit ramp are decoded in a 16-bit plan
- **THEN** the 203 cd/m² patch reads 1.0 within one f16 step, the 1000 cd/m² patch about 4.93, the ramp holds every one of its source steps in order, and no value is clipped
- **TEST** t3881 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: An SDR source's white is 1.0 and its bits are exact everywhere
- **WHEN** an SDR BT.709 fixture is decoded in a 16-bit plan in a scalar and a row-parallel run, and its table output is compared with a stored frame
- **THEN** its white is 1.0, its black 0.0, and the f16 bytes equal the stored frame in both runs
- **TEST** t3882 (`src/libs/kt-ffmpeg/src/colour.rs`)

#### Scenario: Two precisions do not share a cache entry
- **WHEN** the same clip is shown by an 8-bit and a 16-bit plan
- **THEN** the frames cache holds two entries of the two formats, each counted by its bytes in the ledger, and the bars of each plan draw only its own
- **TEST** t3883 (`src/libs/kt-cache/src/lib.rs`)
