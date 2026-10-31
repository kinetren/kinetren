## ADDED Requirements

### Requirement: A forced encoder outside the system's cascade is a named error
When an export forces a video encoder that this system's cascade never tries (`h264_vaapi` on
Windows, `h264_mf` on Linux), the export SHALL fail before writing a file, with an error naming
the encoder and the cascade it is not part of ("`h264_vaapi`: not an encoder this system tries
(h264_nvenc, …)"). Why: the loop over the cascade skipped every candidate and the error read
"no video encoder could be opened (last error: )" — on windows-latest t962, which forced VAAPI,
failed on that empty string.

#### Scenario: The other system's encoder is forced
- **WHEN** an export forces the encoder of the other system's cascade
- **THEN** it fails, the error names the encoder and lists this system's cascade, and no file is
  left behind
- **TEST** t1063 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A forced VAAPI on a system without it
- **WHEN** t962 forces `h264_vaapi` on a system whose cascade has no VAAPI (Windows)
- **THEN** the error names that, and the device half of the test (which needs VAAPI in the
  cascade) does not apply; on Linux the device half runs as before
- **TEST** t962 (`src/libs/kt-engine/tests/export_options.rs`)
