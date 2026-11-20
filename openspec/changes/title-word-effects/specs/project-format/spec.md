## ADDED Requirements

### Requirement: Text-range masks and the isolate mode are written by content
A text-range mask and the isolate mode SHALL raise the `.kt` version by content only when present; a project without
them SHALL be written byte for byte as before; an older build SHALL refuse a file with them naming the version.
Why: an older build would drop the masks on save.

#### Scenario: Round trip and refusal
- **WHEN** a project with a text-range mask in isolate mode is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a project without one is byte-equal to before
- **TEST** t3946 (`src/libs/kt-model/tests/text_range.rs`)
