## ADDED Requirements

### Requirement: A drawn frame asks the disk nothing
A window SHALL NOT ask the disk in the code that draws it: the welcome window SHALL ask whether a recent
project is there once per opening, the question before a render replaces a file SHALL show the size and date
read when it was asked.
Why: found-defects 835's class walk — both asked in every frame, and a frame stands with the disk.

#### Scenario: The replace question shows what was read when it was asked
- **WHEN** a render is started into a file that exists
- **PATH** Render… → Render → pick the existing file → the question names it with its size and date
- **THEN** the question shows that file's size and date
- **TEST** t958 (`src/apps/kt-app/src/reach_tests.rs`)
