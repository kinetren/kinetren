## ADDED Requirements

### Requirement: The import dialog offers every file the import door takes
The dialog of "Import older Kinetren project…" SHALL list `.kt` beside `.otio` and `.ktproj`, because the door takes a
project of the former name and a project with a retired kind, both `.kt`. A refusal by "Open" that names the import
SHALL be followed by a path that works.
Why: the refusal text sends the person to a dialog in which their file could not be seen.

#### Scenario: Open refuses, the import opens
- **WHEN** a project with a retired timecode is opened, then imported from the menu, then saved
- **THEN** Open's error names the kind and the menu item and the open project stays; the dialog's filter holds `kt`; the import shows the title and the notice; Save suggests `<name>-imported.kt`
- **PATH** File → Open… → the old project → the error names "Import older Kinetren project…" → File → Import older Kinetren project… → the same file is in the list → it opens, the digits run → Ctrl+S → the name ends in `-imported`
- **TEST** t3997 (`src/apps/kt-app/src/reach_tests.rs`)
