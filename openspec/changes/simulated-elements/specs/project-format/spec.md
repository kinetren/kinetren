## ADDED Requirements

### Requirement: Element sets and their operators are written by content
An owner's element sets (source, operators, solver rows and curves, drivers, the declared ceiling) SHALL raise the
`.kt` version by content only when present; an older build SHALL refuse such a file naming the version; an owner without
sets SHALL be written byte-equal to before.
Why: an older build would drop the sets on save.

#### Scenario: Round trip and refusal
- **WHEN** a title with a fall solver driven by its outer acceleration, a mask whose vertices spring to a tracker and an emitter are written, read back and offered to a reader of the previous version
- **THEN** they read back equal; the old reader refuses naming the version; a project without sets is byte-equal to before
- **TEST** t3973 (`src/libs/kt-model/tests/element_sets.rs`)
