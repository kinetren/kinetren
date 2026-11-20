## ADDED Requirements

### Requirement: The key is analyzed and inspected where it is edited
The greenscreen's head in the effects window SHALL offer "Analyze"; the node editor's Viewer on a key node
SHALL offer the views Composite, Matte, Status and Screen, shown by the preview only and never rendered; a
legacy-model slot's form SHALL offer "Use the new key", which switches the model in one undo step.
Why: F-MSK-004 trick 1 — a key cannot be tuned without seeing its matte; UI rules 13, 17.

#### Scenario: Key a shot by mouse
- **WHEN** a greenscreen is added from the picker, "Analyze" is clicked, the key node is Ctrl+Shift+clicked and the view set to Status, then Esc
- **THEN** the key is set and the status line says what; the preview shows the exaggerated matte with tinted near-transparent pixels; after Esc the composite returns; an export of the range has no view
- **PATH** effects window → picker → "Greenscreen" → the slot appears → click "Analyze" → the background disappears, the status line names the screen colour → Ctrl+Shift+click the key node → "Viewer: Greenscreen" badge → choose "Status" → black/white matte with tinted dirt → Esc → the composite
- **TEST** t3563 (`src/apps/kt-app/src/node_editor_tests.rs`), planned live scenario `key` (t3564)
