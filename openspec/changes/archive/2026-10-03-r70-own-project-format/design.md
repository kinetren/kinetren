# Design

## Context

The motivation, the measurement and the reconnaissance — `proposal.md` and
`docs/features/project/structure/project-file-vs-interchange.md` (F-PRJ-005). Here only
what is needed for the approach.

What already exists and works for us:

- `kt_model::Project` derives `Serialize`/`Deserialize` and makes the round trip **field by field**
  (verified by measurement on 02.10.2026). The model is already self-contained: `Project` carries both
  the root edit and `sequences` — the bodies of the reels lie in it ONCE, and an instance refers to them by the
  `MediaId` address. So "each entity once" is a property of the MODEL, which is lost
  only when translating to OTIO.
- Writing is already atomic and protected by the revision guard (`write_project_guarded`,
  `SaveOutcome`), and this does not change.
- The import door already exists and is named (R.67, `File → Import older Kinotrain project…`),
  and the rule "a fix for an old file goes into the IMPORT, not into the reader" is already written down.

What gets in the way:

- `kt_otio::Retained` (the passthrough of a foreign file) is assembled by hand and serde does not derive it: none of its types
  (`RawObject`, `MarkerNode`, `TrackExtra`, `ClipExtra`, `MediaRefs`,
  `TransitionExtra`, `RationalTime`) has `Serialize`.
- `.otio` is hard-wired into the application paths in a dozen or more places: dialog filters, the recovery suffix, extension detection on drop, the golden fixtures of the tests.

Limits that do not move: the model does not change (the format changes), atomicity and the revision guard remain, the rule "one cannot silently lose someone else's work" remains, and the live check remains the verdict.

## Goals / Non-Goals

**Goals:**
- A project file the size of the model: a second instance of a reel adds to the file a reference, not a body.
- Two explicit doors to OTIO — import and export, both named, both verified live.
- Autosave stops being an event.
- An unknown version is an honest refusal, not a partial read.

**Non-Goals:**
- Compressing the project file: at units of kilobytes it solves nothing and costs a dependency.
- A database instead of a file (the Resolve path) — another round and another price.
- Splitting the project into parts (the Adobe "Productions" path) — treating the symptom.
- Changing the model. The format is a way of writing the model, not a reason to edit it.

## Decisions

### 1. The format is serialization of the MODEL, JSON, LINE-BY-LINE and deterministic

We take what already works: `serde_json` on top of `kt_model::Project`. The field-by-field round trip is already proven by measurement, and "each entity once" comes about not by a trick, but because the model is built that way.

**Text, not binary, and line-by-line, not compact** — the owner's requirement ("I am against binary stuff — our file has to be indexable in git"), and it caught a hole in the first version of the plan. Compact JSON is one line for the whole file: with 50 events, editing one "changes" the whole file, and `git diff`, `git blame` and branch merging are all dead. Line-by-line (pretty) with the same 50 events is 628 lines, and editing one event changes **1 line**. The price is 1.7× in size (11 KB against 7 KB), still hundreds of times smaller than `.otio`.
"Indexable in git" becomes a measurable rule, not a wish: *editing one event changes a handful of lines, not the file* — this is a test, not prose.

Determinism is the second half of the same requirement: the same model must give the same bytes, otherwise every save is a false diff. `Project` consists of `Vec`s and scalars, so the order is deterministic by construction (verified); the only source of disorder is the `HashMap` fields of the foreign passthrough (`Retained`), and when translated to JSON they are written with sorted keys.

Rejected: a binary format (the owner's ban; besides, parsing is not the bottleneck — the bottleneck was the SIZE); compact JSON (one line is not a file for git); compression (there is no point compressing units of kilobytes, and a compressed file for git is the same binary stuff; Premiere's compression on save became exactly a performance problem); its own schema on top of `Value` (a hand-written layer between the model and the file is exactly what `metadata.kt` was).

### 2. The extension is `.kt`, recognition by CONTENT

The file carries its own marker and version at the root (`"kinotrain": <version>`), and `detect` distinguishes
three kinds by content: our own project, an interchange file (`OTIO_SCHEMA`), legacy (`format_version`).
The requirement "the format is determined by content, not by extension" is already in the spec and in force.

The extension `.ktproj` is NOT reused, although detection would cope: two different formats under
one suffix is a trap for a person and for scripts, and the legacy `.ktproj` lives on in
the import. `.ktp` is also rejected (one character longer and adds nothing).

### 3. The foreign passthrough is translated into JSON, not thrown away

`Retained` gets a `to_json`/`from_json` pair inside `kt-otio` and travels in our file as a separate field. Deriving serde on seven hand-written types would be more expensive and would tie the project format to the OTIO types; translating to `Value` reuses the writers/parsers that already exist.

Rejected: dropping the passthrough with a notice. Cheaper, but it violates a rule older than this round — "one cannot silently lose someone else's work"; with a notice it is no longer silent, but the work is still lost, while we have a mechanism for keeping it.

If the translation turns out to cost more than estimated, the fallback path is named in advance: store in the project the ORIGINAL text of the imported interchange file and apply it on export. This is cruder, but it keeps the promise.

### 4. The inline requirement is not cancelled, but gets an ADDRESSEE

"The body of a reel is inlined into EACH instance" remains true — for EXPORT. A foreign reader does not know our conventions, and it needs self-containment. For the project the same property is a multiplicative cost.

This is the answer to why the earlier decision was not a mistake: it was correct for the interchange file, and it was applied to a file that stopped being an interchange file only now.

**Revision: the R.68 variant "expand the attachment by one level" is rejected** — it rested on the premise that "the bodies of the reels already lie in the envelope `kt.sequences`", while `KtSequence` has no clip fields at all (verified 02.10.2026). Truncating the depth would take content away from OUR reader, that is, it would violate exactly what was promised to the owner.

### 5. The doors: what "Open" accepts, what import does, what export does

- `File → Open` accepts ONLY its own format. An interchange file and a legacy file are rejected, with the name of the import item in the error text (rule 19 — the dialog says where to change the decision).
- `File → Import older Kinotrain project…` is one door for everything foreign and old: legacy `.ktproj`, `.otio` of any version of our envelope, a foreign `.otio`. The result opens as a document of ITS OWN format with the "has changes" flag raised; the previous path is not touched (exactly the behaviour of R.67).
- `File → Export → OpenTimelineIO…` — a new named action.

Rejected: silently opening `.otio` "as before". This is exactly "it opened, and then it is unclear what you are working in"; two doors are cheaper than one clever one.

### 6. The version of our own format is a number, refusal forward, migration backward

One number per file. Greater than the current one — a refusal with the number and the address of the door (partial reading is forbidden: "it opened, and part of the edit is missing" is the worst of the three outcomes, F-RES-003). Less — a migration on reading, one-way, as the envelope does today. Breaking changes are explicitly permitted by the owner, so there is no compatibility cascade with a foreign schema here.

### 7. Order of work: format → doors → move → check

First the format with tests of the round trip and of size (it can be checked without the application), then the application's doors, then the move of autosave and recovery, and only then the cleanup of `.otio` from the paths where it is no longer needed. This way each stage can be checked separately, and a red result names its own cause.

## Risks / Trade-offs

- **A foreign editor will no longer open the project file directly** → this is a deliberate trade-off that all the others have made; the export door is named and checked by a live scenario.
- **Our own format is its own responsibility for durability** ("it opened, but a part is missing", F-RES-003) → the version is refused forward, read backward; the round trip is checked field by field and byte by byte; atomicity and the revision guard are not touched.
- **Translating `Retained` to JSON costs more than estimated** → the fallback path is named in decision 3.
- **`.otio` is hard-wired into the application paths** (filters, recovery suffix, detection on drop, golden fixtures) → cleanup as a separate stage AFTER the working doors, so that red results do not mix.
- **The owner's projects are today in `.otio`** → the migration path is the import itself: open by import, save in our format. A live scenario walks this path end to end.
- **Golden tests and fixtures freeze the OTIO writing** → they remain and continue to freeze the EXPORT; this is what is needed.

## Migration Plan

1. Format: writing, reading, version, refusal forward; tests of the round trip, size and atomicity.
2. `Retained` ↔ JSON; the passthrough survives import → project → export.
3. The application's doors: "Open" on its own format, import of `.otio`/legacy, export to OTIO.
4. Autosave, recovery, lock — on its own format.
5. Cleanup of `.otio` from the paths where it has stopped being the project; live scenarios for both doors.

Rollback: each stage is reversible separately. Until stage 3 the application keeps living on `.otio`; until stage 4 — the autosave. The project format does not delete or rewrite foreign files at any stage.
