## ADDED Requirements

### Requirement: A 16-bit precision raises the file by content
The project's and a composition's picture precision SHALL be written only when it is 16-bit. A project
with such a field SHALL be project-file format 8 by content, and an 8-bit project SHALL be written
byte-identical to before. A reader of an older build SHALL refuse a format-8 file naming its version. The
OTIO export SHALL write the precision into `metadata["kt"]`, and "What OTIO export loses" SHALL carry its
row.
Why: the format is set by content (one version per file); a field an older reader ignores would draw a 16-bit
project in 8 bits without a word (hdr-sources-to-sdr D8).

#### Scenario: The precision in the file
- **WHEN** an 8-bit project is saved, then switched to 16-bit with one composition left 8-bit and saved, then exported to OTIO
- **THEN** the first file equals its former bytes, the second is format 8 and round-trips both precisions, and the OTIO file carries the precision in the envelope
- **TEST** t3878, t3879, t3880 (`src/libs/kt-model/tests/project_file.rs`)
