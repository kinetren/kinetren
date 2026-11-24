## ADDED Requirements

### Requirement: A key written before opens with its rows and the new rows raise the file by content
A greenscreen slot of an older file SHALL read with its rows as they are and SHALL be written back with the same text;
the new rows SHALL be omitted at their defaults, and a file holding any of them SHALL take the next free project file
version, which an older build refuses by naming it.
Why: R.70's rules; A22 changes the picture, not the file.

#### Scenario: Old keys keep their text, new rows raise the file
- **WHEN** the golden greenscreen forms, a project with a curve on `tolerance` and the `demo-concat` projects with a key are opened and saved, then a choke of 2 px is set and the project saved
- **THEN** the first files are written byte for byte as before and the curve still drives the key; the last file carries the new version and the older reader refuses it naming the version
- **TEST** t3561 (`src/libs/kt-model/tests/plugin_format.rs`)
