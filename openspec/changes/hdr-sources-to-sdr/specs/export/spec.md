## MODIFIED Requirements

### Requirement: The colour tags are the conversion's
The RGB→YUV conversion of an export SHALL use the chosen matrix (BT.601 or BT.709) and range
(limited or full), and the stream SHALL be tagged with the same matrix and range, with BT.709
primaries and transfer. The default SHALL follow the decode's size rule: BT.709 at 1280 wide or more or taller than 576 lines,
BT.601 below, limited range; a preset or a text form that names a matrix keeps it, and the default
is written `matrix=auto`.
Why: the files carried no tags at all; a player guessed BT.709 for HD and showed different colours
than the preview (F-EXP-006 trick). A BT.601 default disagreed with the program's own reading of
an untagged HD file (found-defects 1054, hdr-sources-to-sdr D4).

#### Scenario: A saturated colour comes back by the tag
- **WHEN** a flat saturated red frame is exported with BT.601 limited, BT.709 limited and BT.601 full
- **THEN** the reader reports the three tag pairs (smpte170m/bt709 matrix, limited/full range,
  BT.709 primaries and transfer); every file reads back as written within 6 codes through the
  decode that reads the tags (source-colour-matrix) — a file converted with one matrix and tagged
  with another would come back 12 or more codes off, so the conversion follows the label
- **TEST** t2806 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: A fractional frame rate and a frame's own time are written as they are
- **WHEN** 48 frames are written at the rate 24000/1001, and five frames at their own times 0, 3000, 9000, 10000, 19000 in a time base of 1/90000
- **THEN** the first stream's rate is 24000/1001 and it lasts 2.002 s (an integer grid wrote 24 and 2.000 s); the second's packets stand at those times, their gaps in the same ratios (`MuxConfig.frame_rate` is a rational, `time_base` and `write_video_frame_at` serve a proxy that keeps its source's times; found-defects 823 — the project's own rate is F-SPD-005)
- **TEST** t3601 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The same settings write the same bytes
- **WHEN** the same project is exported twice with libx265, libvpx-vp9 and prores_ks, each
  with fixed encoder threads
- **THEN** each pair of files is byte-identical (AV1 encoders are not covered)
- **TEST** t2805 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The default follows the size
- **WHEN** a 1920×1080, a 1280×720, a 720×576 and a 640×360 project are exported with the default settings
- **THEN** the first two are converted and tagged BT.709 limited, the other two BT.601 limited, and each reads back as written
- **TEST** t3808 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: An older preset keeps its matrix
- **WHEN** a preset saved with `matrix=bt601` before this change renders a 1080p project
- **THEN** the file is BT.601, and the default preset's text form says `matrix=auto`
- **TEST** t3809 (`src/libs/kt-engine/tests/render_settings.rs`)
