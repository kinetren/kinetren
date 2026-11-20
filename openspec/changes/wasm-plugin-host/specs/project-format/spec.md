## ADDED Requirements

### Requirement: A wasm plugin's kind is written as any plugin kind
A slot, transition or generator of a wasm plugin's kind SHALL be written by its id and its passport rows exactly as a
built-in plugin kind's; no `.kt` version SHALL be raised by using a wasm plugin.
Why: the plugin id is the file tag (spec `plugins`); the file does not say where a kind came from.

#### Scenario: The same text for a built-in and a wasm kind
- **WHEN** a project with a template-wasm gain slot is written, and the same with the slot's manifest marked built-in
- **THEN** the two files are byte-equal and carry the version of the content without the plugin
- **TEST** t3961 (`src/libs/kt-plugin-wasm/tests/host.rs`)
