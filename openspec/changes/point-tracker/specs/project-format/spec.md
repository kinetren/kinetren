## ADDED Requirements

### Requirement: Trackers raise the file by content
A project with trackers SHALL be written with the next free project file version, a second of samples a line per
point; a project without trackers SHALL be written byte for byte as before; the OTIO
export SHALL carry trackers only in the clip's `kt` metadata.
Why: the R.70 rules (line by line, git-friendly).

#### Scenario: Round trip of a tracker
- **WHEN** a project with a two-point tracker of 600 samples and links to it is saved, read back and read by the previous version
- **THEN** the samples and links round-trip byte-stable; one line holds one second of samples; the older reader refuses naming the version
- **TEST** t3525 (`src/libs/kt-model/tests/project_file.rs`)
