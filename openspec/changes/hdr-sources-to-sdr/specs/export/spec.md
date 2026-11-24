## ADDED Requirements

### Requirement: A 16-bit project exports SDR, HDR10 or HLG
An export of a 16-bit project SHALL offer three outputs:
- **SDR:** the display's tone map, then the conversion of "The colour tags are the conversion's";
- **HDR10:** the BT.2020 PQ encoding, tagged with BT.2020 matrix and primaries and SMPTE ST 2084;
- **HLG:** tagged ARIB STD-B67.

HDR10 and HLG SHALL be offered only with an encoder that takes 10 bits, and SHALL be written at 10 bits.
HDR10 SHALL carry the mastering display (1000 cd/m²) and the content light levels measured over the export, written into the
finished file where the container allows and said where it does not. The RGB→YUV
conversion from f16 SHALL round once, so the bytes are equal between runs on one device and within one code of the reference on any.
Why: kkt-26.11.20 A25 — the 16-bit path exists to show and deliver HDR right; the 10-bit encoders exist
in the cascade (hdr-sources-to-sdr D14).

#### Scenario: HDR10 and HLG are written as tagged
- **WHEN** a 16-bit project with an HDR clip is exported as HDR10 with libx265, as HLG with prores_ks, and twice as HDR10
- **THEN** the files read back with the transfer, primaries and matrix written, the HDR10 one carries mastering display and MaxCLL/MaxFALL equal to the measured ones, the two HDR10 runs are byte-equal, and decoding the HDR10 file in a 16-bit plan gives the export's linear picture within the 10-bit quantization
- **TEST** t3893, t3894 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: An 8-bit encoder offers SDR only
- **WHEN** a 16-bit project is exported with h264_nvenc or a VAAPI encoder chosen
- **THEN** the window offers SDR only and says the encoder takes 8 bits, and the SDR file equals the display's tone map within the 8-bit conversion
- **TEST** t3895 (`src/libs/kt-engine/tests/render_settings.rs`)

#### Scenario: The light levels stand in the finished file
- **WHEN** a 16-bit project with one 800 cd/m² patch is exported as HDR10 to MKV, MP4 with the header moved to the front, and a container without the fields
- **THEN** the first two report the measured MaxCLL and MaxFALL when read back and their video packets are unchanged by the rewrite; the third keeps none and the report says so
- **TEST** t4156 (`src/libs/kt-engine/tests/render_settings.rs`)
