## MODIFIED Requirements

### Requirement: The colour tags are the conversion's
The RGB→YUV conversion of an export SHALL use the chosen matrix (BT.601 or BT.709) and range
(limited or full), and the stream SHALL be tagged with the same matrix and range, with BT.709
primaries and transfer. The default SHALL be BT.601 limited — the conversion every export has used.
Why: the files carried no tags at all; a player guessed BT.709 for HD and showed different colours
than the preview (F-EXP-006 trick).

#### Scenario: A saturated colour comes back by the tag
- **WHEN** a flat saturated red frame is exported with BT.601 limited, BT.709 limited and BT.601 full
- **THEN** the reader reports the three tag pairs (smpte170m/bt709 matrix, limited/full range,
  BT.709 primaries and transfer); every file reads back as written within 6 codes through the
  decode that reads the tags (source-colour-matrix) — a file converted with one matrix and tagged
  with another would come back 12 or more codes off, so the conversion follows the label
- **TEST** t2806 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The same settings write the same bytes
- **WHEN** the same project is exported twice with libx265, libvpx-vp9 and prores_ks, each
  with fixed encoder threads
- **THEN** each pair of files is byte-identical (AV1 encoders are not covered)
- **TEST** t2805 (`src/libs/kt-engine/tests/render_settings.rs`)

