## ADDED Requirements

### Requirement: A wand mask raises the file by content
A project with a wand mask SHALL be written with the next free project file version; a project without
one SHALL be written byte for byte as before; the OTIO export SHALL carry it only in the `kt` metadata
like every mask.
Why: the R.70 rules.

#### Scenario: Round trip of a wand
- **WHEN** a project with a wand mask with three seeds, one linked, is saved, read back and read by the previous version
- **THEN** it round-trips byte-stable and the older reader refuses naming the version
- **TEST** t3538 (`src/libs/kt-model/tests/project_file.rs`)
