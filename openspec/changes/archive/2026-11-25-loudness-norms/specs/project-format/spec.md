## ADDED Requirements

### Requirement: The loudness target and the program stage are written by content
The project's loudness target and the program stage SHALL raise the `.kt` version by content only when
they differ from their defaults (the stage's default is none); a project with the defaults SHALL be written byte for byte as before; an older build
SHALL refuse a file with them naming the version.
Why: an older build would drop the program stage on save and change the mix.

#### Scenario: Round trip and refusal
- **WHEN** a project with the target Streaming, a program gain of +5.5 dB and its measured value is written, read back and offered to a reader of the previous version
- **THEN** it reads back equal; the old reader refuses naming the version; a project at the defaults is byte-equal to before
- **TEST** t3966 (`src/libs/kt-model/tests/program_loudness.rs`)

#### Scenario: The exchange file carries them in its envelope
- **WHEN** the same project is exported to `.otio` and imported, and a file whose stage is out of range is imported
- **THEN** the envelope is v19 and reads back equal, the second write is byte-equal; the project at the defaults keeps its version and names neither; the number out of range reads as no stage
- **TEST** t3729 (`src/libs/kt-otio/tests/convert.rs`)
