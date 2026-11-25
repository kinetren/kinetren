# Spec Delta

## ADDED Requirements

### Requirement: The project file is `.ktren`, the former `.kt` goes through import
A project SHALL be saved and offered as `.ktren`. Open, a drop and the editor's argument SHALL refuse a `.kt` or a
`.ktproj` and name the import item; the recents, the recovery list and a start project SHALL import a `.kt` themselves.
An import SHALL leave the original unchanged and open an unnamed document whose save name is the stem with `.ktren`.
kt-cli SHALL read both by content. The installed file association SHALL claim `.ktren` only.
Why: the owner, 2026-11-07: ".ktren breaking change"; `*.kt` is Kotlin's glob and opens projects in code editors.

#### Scenario: A new project is saved as `.ktren`
- **WHEN** a new project is saved with no extension typed, and its recovery file is written
- **THEN** both end in `.ktren`; the content reads back equal; the reader still refuses a foreign root by name
- **TEST** t4259 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: A `.kt` is refused by Open and named by the door
- **WHEN** a `.kt` is dropped on the window, opened by the dialog and given as the argument
- **THEN** each time the window stays as it was and the notice names the import item; nothing is written
- **PATH** drag `old.kt` from the file manager onto the window → the notice "Projects of this format are opened through File › Import older Kinetren project…" → File menu → Import older Kinetren project… → pick `old.kt` → the title shows `old.ktren*` (unnamed) and the timeline is the same → Ctrl+S → the dialog offers `old.ktren` in the same folder → Save → the title shows `old.ktren`, the `.kt` is as it was
- **TEST** t4260 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The recents import, with the media resolved
- **WHEN** a `.kt` is imported from the recents with relative media beside it, and by the start
- **THEN** every file resolves against the original folder, the document is unnamed with the save hint, the recents row keeps its place
- **TEST** t4261 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A recovery file of the former build is offered
- **WHEN** the recovery folder holds one `.kt` and one `.ktren` recovery file
- **THEN** both are listed, newest first, and the `.kt` one recovers
- **TEST** t4262 (`src/apps/kt-app/src/layout_tests.rs`)
