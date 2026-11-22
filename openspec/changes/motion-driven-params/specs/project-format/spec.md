## ADDED Requirements

### Requirement: Outer-speed links are written by content
A link to an outer-speed channel SHALL raise the `.kt` version by content only when present; an older build SHALL refuse
such a file naming the version.
Why: an older build would read the source as unknown and drop the link on save.

#### Scenario: Round trip and refusal
- **WHEN** a project with an outer speed-change link (attack, release, full-at) is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a project without one is byte-equal to before
- **TEST** t3967 (`src/libs/kt-model/tests/motion_links.rs`)
