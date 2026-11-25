# Spec Delta

## ADDED Requirements

### Requirement: What the render will lose is said before it starts
The render window SHALL list, before Start, the events of the rendered range whose picture or sound runs past the end of
their file and the files the index found truncated, with the moment, the missing length and a jump; the start SHALL stay
available. A truncated file met while rendering SHALL be named once in the log with its last whole moment.
Why: the owner, 2026-11-07: told before the render, "а не в конце".

#### Scenario: An event past its file
- **WHEN** a 3 s event reads a 2 s file through a ramp, one ends 5 ms past, one is a still and one is a frame hold
- **THEN** the first is named with 1 s missing at the moment it runs out, the others are not
- **TEST** t4263 (`src/libs/kt-model/tests/lint.rs`)

#### Scenario: The render window names it and jumps
- **WHEN** the person opens File › Render… on a project with two such events and clicks the line
- **THEN** two lines stand above Start, a click closes nothing and moves the playhead to the moment; with no such event there is no group
- **PATH** File › Render… → the window shows "2 events run past the end of their file" with the two lines → click the first → the playhead stands at its moment → Start renders
- **TEST** t4264 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The log names a truncated file
- **WHEN** a file cut in half after its header is rendered
- **THEN** the export succeeds and its log has one line with the file's name and its last whole moment
- **TEST** t4265 (`src/libs/kt-engine/tests/export_options.rs`)
