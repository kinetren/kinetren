## ADDED Requirements

### Requirement: Speed-channel and transport links are written by content
A link to an acceleration, a speed-change or a transport channel, or to a composition's or a morpher's speed, SHALL
raise the `.kt` version by content only when present; an older build SHALL refuse such a file naming the version.
Why: an older build would read the link's source as unknown and drop it on save.

#### Scenario: Round trip and refusal
- **WHEN** a project with a speed-change link (attack, release, full-at) and a transport link is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a project without them is byte-equal to before
- **TEST** t3951 (`src/libs/kt-model/tests/motion_links.rs`)
