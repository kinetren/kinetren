## ADDED Requirements

### Requirement: Transcripts and subtitle tracks raise the file by content
A project with a transcript or a subtitle track SHALL be written with the next free project file version,
one word and one caption per line; a project with neither SHALL be written byte for byte as before. The
OTIO export SHALL carry them only in the `kt` metadata and write a sidecar `.srt` per subtitle track
beside the exchange file.
Why: the R.70 format rules; an older build refuses rather than dropping the text on save.

#### Scenario: Version by content and the OTIO door
- **WHEN** projects without text, with a transcript, and with a subtitle track are saved, read back, and exported to OTIO
- **THEN** the first keeps its version and bytes; the others carry the new version, round-trip exactly, and an older reader refuses them naming the version; the OTIO export writes the metadata and the `.srt`
- **TEST** t3471 (`src/libs/kt-model/tests/project_file.rs`), t3472 (`src/libs/kt-otio/tests/subtitles.rs`)
