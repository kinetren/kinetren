# Spec Delta

## ADDED Requirements

### Requirement: The project file marker is `kinetren`; the former `kinotrain` comes in through the import door

The project writer SHALL put the root key `kinetren` (with the format version as its
value) into every `.kt` file. The reader SHALL accept only that key. A file whose root
carries the former key `kinotrain` SHALL be refused by the reader. The refusal names the
former product name and the import menu item ("Import older Kinetren project…"). The
import door SHALL accept such a file: it renames the key and reads the rest unchanged,
so the result equals reading the same project written with the new key. The source file
MUST NOT change. `kt-cli import` SHALL behave the same, and `kt-cli` commands that read
projects (`render`, `export`, `probe`, `export-otio`) SHALL refuse a former-marker file
with the same hint.

Why: owner's decision (2026-10-21), a full rename to Kinetren including the marker. Per
the owner's rule (R.67), repairs of old files live in the import, never in the reader.

#### Scenario: New marker written, former marker refused by Open and accepted by import
- **WHEN** a project is written; then a copy of it whose root key is renamed to `kinotrain` is read by the reader, by the import door and by `kt-cli import`
- **THEN** the written text starts with `{\n  "kinetren": 1`; the reader refuses the copy with an error naming "Kinotrain" and "Import older Kinetren project…"; the import door and `kt-cli import` produce a project equal to the original; the copy's bytes are unchanged
- **TEST** t828 (`src/libs/kt-otio/tests/convert.rs`), t829 (`src/apps/kt-cli/tests/cli.rs`)
