## ADDED Requirements

### Requirement: Formats and overrides raise the file by content
A project with formats SHALL be written with the next free project file version, each override on its
event; a project without formats SHALL be written byte for byte as before; removing a format SHALL
remove its overrides in one undo step. The OTIO export SHALL write the master edit and carry formats and
overrides only in the `kt` metadata.
Why: the R.70 format rules; an older build refuses rather than dropping framing on save.

#### Scenario: Round trip and the older reader
- **WHEN** a project with two formats, overrides and a hidden event is saved, read, saved again, and given to a reader of the previous version; and the same project is exported to OTIO
- **THEN** the bytes are stable; the older reader refuses naming the version; the OTIO file holds the master edit with the formats in metadata
- **TEST** t3489 (`src/libs/kt-model/tests/project_file.rs`), t3490 (`src/libs/kt-otio/tests/convert.rs`)
