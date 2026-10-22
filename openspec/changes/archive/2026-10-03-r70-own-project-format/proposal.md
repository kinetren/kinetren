# Proposal

## Why

Kinotrain keeps the project in an INTERCHANGE format (OpenTimelineIO), and pays for this with exactly
what an interchange format does not protect against: **object identity does not exist in OTIO JSON**. There is no way
to say "this composition is the same one as above"; only the nested content can be expressed, so the body of a reel is written anew into EACH of its instances. The owner's three
reels produced **15 220 clips and 115 MB** per file, and autosave writes this volume regularly.

Measurement (02.10.2026, synthetic scenes; the model goes through serde with a field-by-field round trip):

| scene (reel clips / instances / instances) | `.otio` | model serialization | gain |
|---|---|---|---|
| flat project without repeats | 33 KB | 2.0 KB | 17× |
| 10 / 4 / 4 | 496 KB | 2.8 KB | 176× |
| 10 / 8 / 8 | 1.97 MB | 3.9 KB | **501×** |
| 40 / 8 / 8 | 7.36 MB | 7.9 KB | **929×** |

Reconnaissance (`docs/features/project/structure/project-file-vs-interchange.md`, F-PRJ-005):
**not a single major editor lives in an interchange format.** Resolve keeps the project as records in a
DATABASE, and writes a `.drp` only on an explicit command from a person; FCP hides an SQLite
database inside a `.fcpbundle`; Premiere has its own container, and there the price of a mistake is public — compression on
save takes 30–50 %, runs on one core, and is turned off for the sake of speed. Adobe answers growth not with a format,
but by splitting the project ("Productions"), and handles ONE level of nesting well — the same wall we ran into.

The owner's decision (02.10.2026), verbatim: "we are moving away from the otio format, leaving otio only for export, and making breaking changes so that our format becomes a hundred times better".

## What Changes

- **The project lives in ITS OWN format** (`.kt`): serialization of the model, where each entity is
  written ONCE, and a reel instance is a reference to its address. The growth of §464 disappears by construction, rather than being plugged.
- **The file is text, line-by-line and deterministic — "indexable in git"** (the owner's requirement: "I am against binary stuff"). This is measurable: editing one event changes a few
  lines, not the file; the same model gives the same bytes. Compact JSON is rejected —
  one line for the whole file has no meaningful diff.
- **The requirement "the body inline in EACH instance" is not cancelled, but is RE-ADDRESSED
  to EXPORT.** A foreign reader needs self-containment — it does not know our conventions;
  the project needs a reference. These are two opposing requirements for two different files, and
  the attempt to sit on both chairs gave 115 MB.
- **OTIO keeps two doors, and both are explicit**: `File → Export → OpenTimelineIO…` (exchange) and
  `File → Import older Kinotrain project…` (the owner's existing `.otio` projects and the
  legacy `.ktproj` — the same door as for old versions, the R.67 rule). The ordinary "Open"
  accepts only its own format and rejects a foreign one with the name of the import item (rule 19).
- **Autosave, the recovery file and the lock** move to our own format: writing megabytes
  every half minute is both interface lag and a window of loss on a crash.
- **The version of our own format is one; breaking changes are explicitly permitted** (the
  owner's decision). An unknown version is an honest refusal with the number and the address of the door, not a
  silent loss.
- **The foreign passthrough is not lost**: the remnants of the imported `.otio` (`Retained`) travel in
  our file as a separate field and return on export. One cannot silently lose someone else's work — this is a rule older than this round.

Not included: compressing the project file (at 8 KB it solves nothing and costs a dependency), a database instead of a file (the Resolve path — another round and another price), splitting the project into
parts ("Productions"), changing the model — the format changes, the model does not.

## Capabilities

### New Capabilities

(none — the project file is already described by the requirements of `project-format`; WHAT this file is changes)

### Modified Capabilities

- `project-format`: a requirement "The project file is its own format" is added (what it is, the version, refusal on an unknown one, atomic writing); the requirement "The project file is OTIO `Timeline.1`" is rewritten into "The EXCHANGE file is OTIO": the remaining OTIO requirements stay in force, but describe the import/export door, not the project; the requirement "Nested reels — the body inline" gets an explicit addressee (export), while the project stores the reel once.
- `durability`: autosave, the recovery file and the `Drop` barrier write their own format; the outcome of writing and atomicity do not change.
- `compositions`: the requirement "Format — `Stack` as a child of `Track`, the sidecar is advisory" is redirected to export; in the project the instance is a reference to the address of the reel.

## Impact

- A new crate or module of the project format (reading, writing, version, refusal) —
  `src/libs/kt-model` (serialization already exists) plus an entry point that does not exist today.
- `src/libs/kt-otio` — changes its role: it reads only in import, writes only in export;
  `Retained` gets translation to JSON and back.
- `src/apps/kt-app` — `File → Open`/`Save`, autosave and recovery, the new export item,
  dialog extensions; `src/apps/kt-cli` — the same doors for headless use.
- `openspec/specs/{project-format,durability,compositions}/{spec,design}.md`; new tests
  from **t810**; live scenarios for both doors.
- The owner's existing `.otio` projects open by import and are saved in our own
  format — this is the migration path itself.
