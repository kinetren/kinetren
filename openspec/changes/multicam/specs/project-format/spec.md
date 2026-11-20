## ADDED Requirements

### Requirement: Multicam raises the file by content
A project with a multicam composition or an angle choice SHALL be written with the next free project
file version; a project without them SHALL be written byte for byte as before. The OTIO export SHALL
write each instance piece's nested stack with only its active angle's tracks enabled and the selection
in the `kt` metadata.
Why: the R.70 rules; a foreign editor at least plays the right angle per piece.

#### Scenario: Round trip and the OTIO door
- **WHEN** a project with a multicam and three switched pieces is saved, read back, read by the previous version, and exported to OTIO
- **THEN** it round-trips byte-stable; the older reader refuses naming the version; the OTIO stacks enable only the active angle per piece
- **TEST** t3511 (`src/libs/kt-model/tests/project_file.rs`), t3512 (`src/libs/kt-otio/tests/convert.rs`)
