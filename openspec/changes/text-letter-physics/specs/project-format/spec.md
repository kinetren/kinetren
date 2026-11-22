## ADDED Requirements

### Requirement: The Physics animator is written by content
A title's Physics animator rows SHALL raise the `.kt` version by content only when set; an older build SHALL refuse a file
with them naming the version.
Why: an older build would drop the animator on save.

#### Scenario: Round trip and refusal
- **WHEN** a title with the Physics animator (behaviour fall, a stiffness curve) is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a title without it is byte-equal to before
- **TEST** t3973 (`src/libs/kt-model/tests/letter_physics.rs`)
