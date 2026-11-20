## ADDED Requirements

### Requirement: A running title is written by content
A title's source, format, start and frame rate SHALL be written only when they differ from the typed
text's defaults, so a project without a running title is written byte for byte as before; a project with
one SHALL be format 7, which an older build refuses by its version instead of showing and saving the typed
text. The source SHALL travel through the `gen:` path as well.
Why: an older build reads the title without the new fields and would save it without them (the rule of
marker-notes and track-device-sound: the version by content).

#### Scenario: Written by content
- **WHEN** a countdown title round-trips through its gen path and serde; a typed title is serialized; a project holds a typed title, then a countdown title
- **THEN** both round trips return the same title; the typed title writes no new field; the project is format 1, then format 7
- **PATH** save a project with a typed title → the file is as before; switch it to Countdown and save → the file says format 7
- **TEST** t3704 (`src/libs/kt-model/tests/text_source.rs`)
