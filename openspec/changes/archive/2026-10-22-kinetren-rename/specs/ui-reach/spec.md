# Spec Delta

## MODIFIED Requirements

### Requirement: Project file doors — "Open" only accepts our own format, import through one door, export as a named action
"Open" (the menu, launching with a file, dropping onto the window) SHALL accept only a PROJECT file
(`.kt`). An interchange file `.otio` and a legacy `.ktproj` SHALL be rejected with the kind of file and the name of the
import item in the error text (rule 19), each with its own text; a file of a newer format
version — with the version number and without partial reading. The item "Import older Kinetren
project…" SHALL be ONE door for four inputs — a legacy `.ktproj`, our `.otio` of any
envelope version, a foreign `.otio`, and a `.kt` with the former marker `kinotrain` (before the rename
to Kinetren, 2026-10-21); the result SHALL open as an UNNAMED document of its own
format with the "has changes" flag raised and a hint "save as `<name>.kt` next to the
source", the source MUST NOT change; a project file with the current marker does not go through this door — the error
names "Open"; "Open" rejects a file with the former marker with the name Kinotrain and the
import item. Export to OTIO SHALL be a separate item `File → Export →
OpenTimelineIO…`, named by industry convention; it MUST NOT change the project file, its path or its
asterisk. Files dropped on the window — `.kt`, `.otio` and `.ktproj` — SHALL lead to three different outcomes.
The dialog filters SHALL know the `.kt` extension; the import filter — both foreign ones.
Why: the owner's decision (R.70) — "we are moving away from the otio format, leaving otio only for
export"; a document after import has no project file, and one may not write `.kt` under a foreign name or silently put it next to the source — the person chooses the name in the dialog; the second item
"Import .ktproj…" was an extra door with the same meaning.

#### Scenario: "Open" accepts only its own format and names the door
- **WHEN** "Open" receives a `.kt` file; separately — an exported `.otio`, a legacy `.ktproj` and a `.kt` with a version greater than the current one
- **THEN** the first opens field by field with the project path; the second and third are rejected with the name of the import item and the kind of file (the texts differ); the fourth — with the version number and without the import item name
- **PATH** File → Open → `film.otio` → error window: "an OpenTimelineIO file, not a project — open it via File / Import older Kinetren project…" → File → Open → `film.kt` → the timeline with the edit, the project name in the title
- **TEST** t818 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: One import door for three inputs
- **WHEN** through "Import older Kinetren project…" are fed in turn a legacy `.ktproj`, our `.otio` of an old envelope version and a foreign `.otio` with unknown fields; then a `.kt` file
- **THEN** each of the three opens as a document without a path, with a hint to save as `<name>.kt` next to the source and with an asterisk; the source is byte for byte the same as before; the foreign fields are in the passthrough; in the "File" menu there is one door (there is no item "Import .ktproj…"); the `.kt` file is rejected with the name "Open"
- **PATH** File → Import older Kinetren project… → `old.otio` → the timeline with events, in the title "untitled *" → Ctrl+S → the dialog proposes `old.kt` next to the source
- **TEST** t819 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/import_old.py`

#### Scenario: A project saved under the former name
- **WHEN** "Open" gets a `.kt` whose root key is the former `kinotrain`; then "Import older Kinetren project…" gets the same file
- **THEN** "Open" refuses it with a text naming Kinotrain as the former name and the import item; the import opens it as an untitled document with the star and the `<name>.kt` save hint, events and tracks equal to the file; the file's bytes are unchanged
- **PATH** File → Open → `old.kt` → error window: "a project of Kinotrain (the former name) — use File → Import older Kinetren project…" → File → Import older Kinetren project… → `old.kt` → timeline with the edit, title "untitled *"
- **TEST** t832 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Export to OTIO from the menu
- **WHEN** the "File" menu → the "Export" submenu → "OpenTimelineIO…" is opened and a file is named; the project at this time has unsaved edits and a foreign passthrough, and a reel in two instances
- **THEN** the item is reachable and named "OpenTimelineIO…"; the interchange file is written: `Timeline.1`, two `Stack`s with non-empty bodies, the foreign fields returned; the project file is byte for byte the same as before, the path and the asterisk are untouched; the notification names the export path; "Open" rejects this file
- **PATH** File → Export → OpenTimelineIO… → the save dialog → file name → the notification "exported: …", the asterisk in the title as it was
- **TEST** t820 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/export_otio.py`

#### Scenario: Three files on the window — three outcomes
- **WHEN** `film.kt`, `film.otio`, `film.ktproj` are dropped in turn on the window
- **THEN** the first gives the question "open or insert as a composition", the edit is untouched; the second — a notification with the kind of file (OpenTimelineIO) and the name of the import item, without a question; the third — a notification about the legacy format with the same item; the texts of the second and third differ
- **PATH** drag `film.otio` onto the window → in the bell: "an OpenTimelineIO file, not a project — open it via File / Import older Kinetren project…" → drag `film.kt` → the window "Project dropped: open or insert as a composition?"
- **TEST** t821 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Import of an old-version project — its own door; "Open" rejects it with the address
The "File" menu SHALL have the item "Import older Kinetren project…" (the industry name —
"Import project", as in Premiere/Resolve), which reads a file of the envelope of a PREVIOUS version,
applying repairs to the old forms, and opens it as an UNNAMED document of the current form
(R.70: of its own format `.kt`) with the "has changes" flag raised and a hint to save as
`<name>.kt` next to the source (the next write goes in the new form; the source is not
touched). The ordinary "Open" SHALL reject such a file with the file's version and with the name of the
import item in the error text (rule 19: the dialog says where to change the decision), and not with
"metadata.kt is unreadable". The item SHALL be in the verb registry and reachable from the menu and from
the open-error window.
Why: the owner's decision (R.67): "make an item in the File menu for importing old versions, and fix things in the import itself so the reader can read it normally". R.70: the former path
no longer suits the document — it points to the exchange file, while the project is written in its own
format under its own name.

#### Scenario: An old file through import, "Open" names the door
- **WHEN** a v5 file with `targets: null` on the morphers (the form `untitled2_v26.09.26.otio`) is opened via "Open"; then via "Import older Kinetren project…"
- **THEN** "Open" rejects it with a text that contains the file's version (5) and the name of the import item; the import opens the project: morphers with `null` act on "everything below", the number of events and morphers equals the file's, the document has no path and carries a hint `<name>.kt`, the title carries an asterisk, saving writes its own format and the next "Open" already reads the file itself
- **PATH** File → Open → old file → error window: "version 5; use File → Import older Kinetren project…" → File → Import older… → the same file → the timeline with events and morphers, in the title "untitled *"
- **TEST** t783 (`src/apps/kt-app/src/reach_tests.rs`), t783b (`src/libs/kt-otio/tests/convert.rs`), live scenario `src/scripts/live/import_old.py`
