# Spec Delta

## MODIFIED Requirements

### Requirement: Media paths — relative, file:// fallback, pseudo-paths verbatim
`target_url` SHALL be written relative to the project file with POSIX slashes; a path that cannot be made relative
(only the root is shared) — as an absolute `file://` URL with percent-encoding; all
three kinds (relative, absolute, `file://`) SHALL be read. Pseudo-paths (`synth:`, `gen:`) SHALL
travel verbatim. The path of media that is ADDED to the project (dialog, drag-and-drop, command line)
SHALL be made absolute relative to the current process directory AT THE MOMENT
OF ADDITION; the model has no relative file paths, relativity appears
only on writing and only relative to the project file. A project file without a path (not saved)
SHALL keep its paths absolute until the first write.
Why: the owner's decision "relative; fallback/absolute — only if this exists in
other editors" (Resolve/Hiero write absolute `file://`). R.67: in the owner's project
the picture is recorded as `sources/a3566951817_10.jpg` relative to a file lying in
`sources/` — a path relative to the launch directory got into the model as is, and on
writing it was taken as relative to the project; the file "vanished" without moving.

#### Scenario: Relativization, encoding and resolution
- **WHEN** a path next to, deeper than, above and on another root is relativized; a path with spaces/percent signs/UTF-8 is encoded into a URL and back; `synth:` and an already relative path pass through the writer
- **THEN** next to/deeper/above give relative paths, another root gives a `file://` URL; the URL decodes to the original path; `synth:` and the relative path are not changed; resolution accepts all three kinds
- **TEST** t147 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: An added relative path becomes absolute
- **WHEN** media is added by the path `sources/x.jpg` with the current directory `/work` into an unsaved project; the project is saved to `/work/sources/p.otio`; it is opened again
- **THEN** in the model the path is `/work/sources/x.jpg`; in the file — `x.jpg` (relative to the project); after opening the file is found (on the previous code — `sources/x.jpg` relative to the project, that is `/work/sources/sources/x.jpg`, a disappearance)
- **TEST** t797 (`src/apps/kt-app/src/layout_tests.rs`), t797b (`src/libs/kt-otio/tests/format.rs`)

## ADDED Requirements

### Requirement: Import of an old-version project — a separate door; fixes lapse with versions
A project file of a version below the current one, which the ordinary reader rejects, SHALL be opened
through IMPORT: a separate entry that applies to the document REPAIRS of known old forms
before the ordinary read and gives the project of the current form plus a list of the repairs made
(as notices). The reader MUST NOT learn new old forms: a repair goes into the import, each
— with the version it was made for, and is DELETED when support for that version
is dropped (the list of versions-with-repairs is one place). Ordinary opening of such a file SHALL
be rejected with an error naming the file's version and pointing to the import. The first repair: `targets:
null` of a v5 morpher ⇒ "everything below the line". The import does not write the source file; the next
write goes in the current form and version.
Why: the owner's decision (R.67) — "fix it precisely in the import… and also pin it for other agents so that
they automatically make the fixes into the import of old versions themselves; these fixes will gradually be deleted
as the old versions go away"; a reader with accumulated eternal migrations is exactly the "backward
compatibility" that "is not needed".

#### Scenario: Import repairs v5, the reader is untouched
- **WHEN** a v5 file with `targets: null` on three morphers, an object `{on: clips}` on two and a list on one is read by the ordinary reader; then by the import; then the imported project is written and read by the reader
- **THEN** the reader rejects it with version 5 and the word "import"; the import gives a project with six morphers: three — "everything below", two — by list, one — by list; in the notices — "morpher targets: null → below (v5)" ×3; the written file is read by the reader without repairs (the version — by content, `null` does not remain in it); the list of repairs in the code carries a version for each
- **TEST** t783b (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Segment formula and formula layer in the file — v13 by content
A key with the "segment formula" form and a curve with a formula layer SHALL be written into the curve envelope;
the envelope version SHALL rise to 13 ONLY when the file contains at least one of them;
a file without them SHALL be written with the previous bytes and the previous version. An old build on such a
file SHALL refuse by version (a segment form it does not know would sound like a direct one,
the layer would stay silent), rather than read it partially.
Why: the rule "one version, raised by content" (v9–v12); silent reading of an unknown form is a spoiled edit without a sign.

#### Scenario: Round trip to v13 by content
- **WHEN** a project with a segment formula is written and read; a project with a layer — the same; a project without them is written after opening a v12 file
- **THEN** the first two — v13, the round trip byte for byte, the forms and the layer are the same; the third — v12 and the previous bytes; the v12 reader on a v13 file refuses by version
- **TEST** t787b, t787d (`src/libs/kt-otio/tests/convert.rs`)
