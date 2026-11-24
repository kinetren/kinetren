## ADDED Requirements

### Requirement: Fit the preview is a command of the controls table
"Fit the preview" SHALL be a command of the closed list with a frozen number, bound to Shift+0 in every style and shown in
the preview's zoom menu with its key; egui's interface zoom SHALL keep Ctrl+0.
Why: one table for keys and labels (rule 17); a settings code given before this command must still open.

#### Scenario: The command and the old code
- **WHEN** the key list is read in each style, Shift+0 and Ctrl+0 are pressed, and a settings code made before this command is entered
- **THEN** the command stands in View with Shift+0 in every style; Shift+0 fits the preview and Ctrl+0 resets the interface zoom; the old code opens with the command at its default
- **PATH** Help → Keys → the View group holds "Fit the preview — Shift+0" → zoom the preview out → Shift+0 → it is whole; Ctrl+0 → the interface returns to its own size
- **TEST** t4229 (`src/apps/kt-app/src/layout_tests.rs`), t1702 (`src/apps/kt-app/src/controls_tests.rs`)
