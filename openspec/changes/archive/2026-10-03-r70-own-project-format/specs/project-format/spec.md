# Spec Delta

## ADDED Requirements

### Requirement: Project file — its own format: each entity written once
The project SHALL be read and written in ITS OWN format (`.kt`), not in the interchange format. Each
entity of the document SHALL be written ONCE: a reel is stored with its body at an address, and
its instance on the edit is a reference to that address. The size of the file SHALL depend on the size of the
MODEL, not on the number of instances: adding a second instance of the same reel MUST NOT
increase the file by the size of its body.
The file SHALL carry the VERSION of the format and a marker by which it is recognised by content, not by
extension. An unknown (newer) version SHALL be rejected with an explicit error giving the version
number and the name of the item with which the file can be opened — and it MUST NOT be read partially:
"it opened, and part of the edit is missing" is worse than an honest refusal. Breaking changes of the format
are PERMITTED (the owner's decision): a compatibility cascade with a foreign schema does not apply here.
The foreign passthrough remnants of an imported interchange file SHALL travel in the project as a separate
field and return on export: one cannot silently lose someone else's work.
The file SHALL be TEXT, LINE-BY-LINE and DETERMINISTIC — "indexable in git"
(the owner's requirement: "I am against binary stuff"). This is measurable: editing one event SHALL
change a few lines, not the file; the same model SHALL give the same bytes; in the foreign
passthrough, unordered maps (`HashMap` fields) SHALL be written with sorted keys
so that their traversal does not become a false diff, while the order of the FOREIGN keys of the document SHALL
be preserved — it is part of the promise to the interchange file. A binary format, compression and a single-line (compact) JSON MUST
NOT be applied to the project file.
Writing SHALL be atomic and idempotent under the same conditions that the interchange file had
(`save(load(save(p))) == save(p)` byte for byte).
Why: in the JSON interchange format there is NO object identity — there is no way to say "this is the same reel as
above"; only the nested content is expressed, and the body of the reel is written anew into
each of its instances. For the owner, three reels gave 15 220 clips and 115 MB per file, which
autosave writes regularly. Measurement: `.otio` against model serialization — 17× on a flat
project, 501× and 929× on nested scenes. The owner's decision: "we are moving away from the otio format,
leaving otio only for export, and making breaking changes so that our format becomes a hundred times
better".

#### Scenario: A second instance of a reel does not double the file
- **WHEN** a project with a reel is saved; then the same reel is placed as a second and third instance and the project is saved again
- **THEN** the growth of the file for each new instance equals the size of a REFERENCE, not the body of the reel; for a nested scene (a reel inside a reel, with several instances each) the file stays orders of magnitude smaller than the same project in the interchange format
- **TEST** t810 (`src/libs/kt-model/tests/project_file.rs` — growth per instance; `src/libs/kt-otio/tests/project_file.rs` — against the interchange file)

#### Scenario: The project's round trip field by field and byte by byte
- **WHEN** a project with reels, morphers, cameras, transitions, public parameters and foreign passthrough remnants is written, read and written again
- **THEN** the read model equals the original field by field, the second round trip is byte for byte, and the passthrough is intact
- **TEST** t811 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The version of its own format — refusal, not a partial read
- **WHEN** a file of our own format with a version GREATER than the current one is opened; separately — a file with a lower version; separately — an interchange file `.otio`
- **THEN** the first is rejected with the version number in the error text; the second is read by migration or rejected with the name of the import item; the third is rejected with the name of the import item, and is not read as a project
- **TEST** t812 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The file is indexable in git
- **WHEN** a project of about fifty events is saved; then one event is shifted and the project is saved again; separately — the same model is saved twice in a row; separately — a project with foreign passthrough is saved twice
- **THEN** the file is multi-line (no fewer lines than events), the line-by-line difference between two saves is a handful of lines; two saves of one model are byte for byte equal, and with the passthrough too
- **TEST** t822 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The write is honest about power loss
- **WHEN** writing a project is interrupted between the temporary file and the rename
- **THEN** either the previous project remains on disk whole, or the new one whole; the outcome of writing is a type, and a refusal does not look like success
- **TEST** t813 (`src/libs/kt-otio/tests/project_file.rs` — the atomic writer lives there)

### Requirement: Export to OTIO — an explicit door, a self-contained file
Export to OTIO SHALL be a SEPARATE named action (`File → Export → OpenTimelineIO…`),
not a save. The exported file SHALL be SELF-CONTAINED: the reader does not know
our conventions, so the body of a reel is inlined into each of its instances, and our envelope
remains advisory. The ordinary "Open" MUST NOT accept an interchange file: it SHALL
be rejected with the name of the import item in the error text (rule 19).
Why: self-containment is needed by the exchange and harmful to the project — these are opposing requirements
for two different files. Not a single major editor lives in an interchange format: Resolve
keeps the project as records in a database and makes a `.drp` only on a person's command; FCP hides
an SQLite database inside a bundle (reconnaissance F-PRJ-005).

#### Scenario: The export is self-sufficient, opening does not accept it
- **WHEN** a project with a reel in three instances is exported to OTIO; then the resulting file is offered to the ordinary "Open"
- **THEN** the export has three `Stack`s with NON-EMPTY bodies and the previous envelope version; "Open" rejects it with the name of the import item
- **TEST** t814 (`src/libs/kt-otio/tests/convert.rs`)

## MODIFIED Requirements

### Requirement: Project file — OTIO `Timeline.1`, format detected by content
**Since R.70 this requirement describes the EXCHANGE FILE, not the project file** (the project is a separate
requirement "The PROJECT file is its own format"). The heading is left unchanged on purpose: it is the address
by which the requirement is referenced, and renaming it would lose those references.

The EXCHANGE file SHALL be read and written as one OTIO `Timeline.1` with the `Stack.1` of tracks.
The kind of file SHALL be determined by content (the marker of its own format, `OTIO_SCHEMA`,
`format_version`), not by extension. A root that is not a `Timeline` (for example
`SerializableCollection`) SHALL be rejected with an explicit error. The old `.ktproj` and the interchange file
`.otio` MUST NOT be opened as a project — only through a separate import item, which
converts and opens the result in ITS OWN format, without touching the source.
All other requirements of this capability that describe OTIO (idempotency, passthrough,
schema versions, time in ticks, media paths, model mapping, the version of the `kt` envelope, migrations,
transitions, nested reels, generators, the legacy `.ktproj`, conformance with the reference
library) SHALL relate to the IMPORT AND EXPORT DOORS, not to the project file.
Why: the owner's decision — "as a PROJECT, .ktproj no longer opens; no need to drag crutches along"; one cannot trust the extension, because recovery and tmp files carry other people's suffixes.
R.70: the same argument extends to `.otio` — the interchange format has stopped being the project format, and the door to it is now called by its own name.

#### Scenario: Detection by content and refusal on a foreign root
- **WHEN** the input is JSON of our own format, JSON with `OTIO_SCHEMA`, JSON with `format_version`, or `SerializableCollection.1`
- **THEN** the first is recognised as a project, the second as an interchange file, the third as a legacy project (and loading either of the first and third as a project gives an error with the name of the import item), and the fourth is rejected with an "unsupported root" error
- **TEST** t144, t146 (`src/libs/kt-otio/tests/format.rs`)

### Requirement: Nested reels — `Stack` as the child of `Track`, body inline, sidecar advisory
In the EXCHANGE file an instance of a reel SHALL be written as a `Stack` child of a `Track` (the window — the
`source_range` of the stack, the name — `Stack.name`, the chain and reverse — `Stack.effects`, the address of the reel —
`metadata.kt.seq`), and the body of the reel SHALL be inlined into EACH instance. On reading, the
instances SHALL collapse into one reel by address; diverged bodies SHALL become
separate reels with fresh ids and a notice. The sidecar `metadata.kt.sequences` (tracks,
the order of rows, morphers, cameras, the layer map of the reel) SHALL be advisory: if it is missing,
the edit is read from the file with a notice. Foreign nested `Stack` and `Track` inside a `Track` SHALL
be read as real reels. An event to the left of the zero of the reel SHALL carry `kt.start` only where
its position in the track does not express it (v8).
In the PROJECT file the body of a reel SHALL be stored ONCE, and an instance SHALL be a reference to its
address: inlining is not needed there and is forbidden by its cost — it is what gave 115 MB.
Why: JSON OTIO has no object identity; a silent choice of one of two bodies is the loss of
someone else's work; a foreign editor draws the content of attachments, while a grey placeholder does not.
R.70: the same fact (no object identity) means that for the PROJECT inlining is
not a virtue but a multiplicative cost; the requirement is split by addressee.

#### Scenario: A reel in two instances survives a round trip
- **WHEN** a reel with a crossfade, a morpher layer and a camera stands twice, one instance has level 0.5 and reverse; the file is an EXPORT to OTIO
- **THEN** the file has two `Stack`s with non-empty bodies, version v6; after reading there is one reel, the content field by field, the instances field by field, and the second round trip is byte for byte
- **TEST** t431 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Diverged bodies and a lost sidecar
- **WHEN** the body of the second instance is changed by a foreign editor; the `sequences` sidecar is deleted
- **THEN** the first gives two reels with a notice and a valid project; the second gives a reel with the edit and the crossfade from the file, without the morpher, with a notice
- **TEST** t432, t433 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Foreign attachments become genuine reels
- **WHEN** `nested_example.otio` is read (two `Stack`s and a `Track` inside a `Track`)
- **THEN** three reels with events, `foreign` is empty, after unfolding no event points at a reel, and the round trip is idempotent
- **TEST** t437 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An event to the left of zero inside a reel
- **WHEN** the first event of a reel starts at −10000, the second stands at its own time
- **THEN** `kt.start` is present on exactly the two shifted events, the file is v8, the round trip is byte for byte; an ordinary edit does not carry the field
- **TEST** t638 (`src/libs/kt-otio/tests/convert.rs`)
