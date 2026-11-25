# Design

## Context

`EXTENSION = "kt"` (`project_file.rs:134`) is used in seven places of `app.rs`, one of `welcome.rs`, one test that
asserts the text. `RECOVERY_EXT` and `UNTITLED_PROJECT` are constants beside it. The file's CONTENT is what the reader
checks (`"kinetren"` root), so nothing in the format changes: two names, one format.

## Decisions

### D1. Content decides what a file is, the name decides which door
`from_text` is untouched. `Open`, `open_dropped` and the program's argument look at the extension first: `ktren` →
open; `kt` or `ktproj` → a notice with the menu item's name (the one `KTPROJ_IMPORT_ONLY` uses; the text covers both)
and, from the recents, the import itself. A file with no extension or a foreign one is read by content as today.
`kt-cli` takes either: it is a tool, a script renamed nothing.

### D2. Import of a `.kt` gives an unnamed document
R.70's import semantics: the file is read, the fixes of its version run (`legacy_import` has the table), the document
is unnamed with `save_hint` `<stem>.ktren` in the same folder, the original is untouched, and its media paths resolve
against the ORIGINAL folder. Ctrl+S then asks once, with the new name ready. Rejected: writing the new file silently
beside the old one at the first start (two files nobody asked for), and opening `.kt` in place (his answer says no).

### D3. Recents, last project, recovery, locks
A row of the recents whose file is a `.kt` keeps its place; a click imports. The start project being a `.kt` imports and
says so once in the status line; with the welcome window on, nothing opens by itself, so nothing is said. The recovery
list takes both extensions (a crash before the update must be recoverable after it); new recovery files are `.ktren`.
The project lock is by canonical path: a `.kt` open in an older build does not lock the import. The cache folder is the
project folder's, so it is found by the same folder.

### D4. Association
`com.kinetren.Kinetren.xml`: the project type's glob is `*.ktren`, its magic stays (a renamed file with no extension).
The `.kt` glob leaves our package, which is the defect's whole cure: Kotlin keeps its own.

### D5. Scripts and fixtures
Fixtures and live scenarios write `.kt` projects into their own folders (39 live scripts): `kt_live.py`'s writer takes
the extension from one constant, so one place changes; t925–t935's packaging guards move to the new glob.

## Risks / Trade-offs

- [A person shares a `.kt` with someone on the old build] → nothing changes for them; a `.ktren` opens in no older
  build by Open (its filter is `.kt`), by content it does through its own Import only if the reader knows the format
  version. Said in What's new.
- [A backup script that copies `*.kt`] → unchanged files; new saves are not copied. Said in What's new.

## Open Questions

None: the decision is his.
