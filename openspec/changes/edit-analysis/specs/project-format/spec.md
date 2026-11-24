## ADDED Requirements

### Requirement: Tempo corrections and measure links are written by content
A media's beat correction and a link to a measure SHALL raise the `.kt` version by content; a project with neither SHALL be
written byte for byte as before; an older build SHALL refuse naming the version. Measures themselves SHALL NOT be written:
they are a cache.
Why: an older build would drop the link and render another picture.

#### Scenario: Round trip and refusal
- **WHEN** a project with a correction of two anchors and a zoom linked to `beat` is written, read and offered to the previous reader
- **THEN** it reads back equal, the old reader refuses naming the version, and a project with the switches on alone is byte-equal to before
- **TEST** t4216 (`src/libs/kt-model/tests/beat_fix.rs`)
