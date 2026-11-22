## ADDED Requirements

### Requirement: Roles, multi-track keys and the title rule are written by content
Roles, Auto-ducking rules and keys of several tracks or roles SHALL raise the `.kt` version by content only when set; a
project with R.65's single track key SHALL read as a key set of that one track with its text unchanged; an older build
SHALL refuse a file with the new rows naming the version.
Why: an older build would drop them on save; R.65's projects stay as they are.

#### Scenario: By content, round trip, refusal
- **WHEN** an R.65 project is read and written; then a project with roles, a two-track key and "Mute" for titles is written, read back and offered to a reader of the previous version
- **THEN** the R.65 file is byte-equal after the round trip; the new one reads back equal with the raised version; the old reader refuses naming it
- **TEST** t3918 (`src/libs/kt-model/tests/roles.rs`)
