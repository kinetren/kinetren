## ADDED Requirements

### Requirement: Plugins are added from a file or the Library, and a fault is named
The Library window SHALL list plugins as items (installed, available, update); "Add a plugin…" SHALL install a
`.ktplugin` package after its trial and say that it works from the next start, with a button that restarts; a plugin switched off by a fault SHALL be named once in the notices with
the reason.
Why: the owner, A11 — "сделат библиотеку плагинов".

#### Scenario: Add a plugin from a file and use it
- **WHEN** the person adds the template package from a file, restarts and opens the effect picker
- **THEN** the plugin's kinds appear in their categories with a "plugin" mark; adding one works as a built-in
- **PATH** View → Library… → "Add a plugin…" → choose `template.ktplugin` → the row "Template (plugin) 1.0" appears as installed, "works after a restart" → "Restart now" → open an event's effects → picker → the "Template gain" card with a plugin mark → click it → the node appears
- **TEST** t3960 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `wasm_plugin`
