## ADDED Requirements

### Requirement: Roles and role keys raise the file by content
A track's or event's role and a compressor's role key SHALL be written to the `.kt` file, raising its version (the
next free number when applied) only when one is set; a project without them SHALL be written byte for byte as
before; an older build SHALL refuse a file that has them, naming the version. The `.otio` export SHALL carry them
only in `metadata["kt"]`.
Why: an older build would drop the roles on its next save; the OTIO rule of the CORE section.

#### Scenario: By content, round trip, refusal
- **WHEN** a project without roles is written; then one event gets the role Dialogue and a track a Roles key, and it is written, read back and offered to a reader of the previous version
- **THEN** the first file is byte-equal to before; the second has the raised version and reads back equal; the old reader refuses with the version named
- **TEST** t3918 (`src/libs/kt-model/tests/roles.rs`)
