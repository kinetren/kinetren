# Project format

## Purpose

The PROJECT file is its own format `.kt` (R.70): a serialization of the model as line-by-line deterministic JSON, where each entity is written once, the marker and the version are at the root, refusal on an unknown version, atomic write; the file is indexable in git.
The EXCHANGE file is OpenTimelineIO (.otio, JSON), behind two named doors (import and export): standard schemas for everything the format can express, lossless passthrough of foreign data, our data in `metadata["kt"]` with a version by content, time only in ticks, idempotent writing.
Everything in this capability that speaks about OTIO refers to the DOORS, not to the project file.

## Requirements

### Requirement: Export to OTIO — an explicit door, a self-contained file
Export to OTIO SHALL be a SEPARATE named action (`File → Export → OpenTimelineIO…`), not a save. The exported file SHALL be SELF-SUFFICIENT: the reader does not know our conventions, so the body of a reel is inlined into each of its instances, and our envelope remains advisory. A regular "Open" MUST NOT accept an exchange file: it SHALL be rejected with the name of the import item in the error text (rule 19).
Why: self-sufficiency is needed by exchange and harmful to the project — these are opposite requirements for two different files. No major editor lives in an exchange format: Resolve keeps the project as a database record and makes `.drp` only on the person's command; FCP hides a SQLite database inside the bundle (reconnaissance F-PRJ-005).

#### Scenario: The export is self-sufficient, opening does not accept it
- **WHEN** a project with a reel in three instances is exported to OTIO; then the resulting file is offered to an ordinary "Open"
- **THEN** the export has three `Stack`s with NON-EMPTY bodies and the previous envelope version; "Open" rejects it with the name of the import item
- **TEST** t814 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Project file — its own format: each entity written once
The project SHALL be read and written in ITS OWN format (`.kt`), not in the exchange format. Each entity of the document SHALL be written ONCE: a reel is stored with its body at an address, and its instance on the edit is a reference to that address. The size of the file SHALL depend on the size of the MODEL, not on the number of instances: adding a second instance of the same reel MUST NOT increase the file by the size of its body.
The file SHALL carry a FORMAT VERSION and a marker by which it is recognized by its content, not by its extension. An unknown (newer) version SHALL be rejected with an explicit error carrying the version number and the name of the item by which the file can be opened — and MUST NOT be read partially: "it opened, and part of the edit is missing" is worse than an honest refusal. Breaking changes of the format are ALLOWED (the owner's decision): a compatibility cascade with a foreign schema does not apply here.
Foreign passthrough remnants of an imported exchange file SHALL travel in the project in a separate field and return on export: foreign work must not be silently lost.
The file SHALL be TEXTUAL, LINE-BY-LINE and DETERMINISTIC — "indexable in git" (the owner's requirement: "I am against binary junk"). This is measurable: editing one event SHALL change units of lines, not the file; one and the same model SHALL give the same bytes; for foreign passthrough, unordered maps (`HashMap` fields) SHALL be written by sorted keys so that their traversal does not become a false diff, while the order of the FOREIGN keys of the document SHALL be preserved — it is part of the promise to the exchange file. A binary format, compression and single-line (compact) JSON MUST NOT be applied to the project file.
The write SHALL be atomic and idempotent under the same conditions as the exchange file had (`save(load(save(p))) == save(p)`, byte for byte).
Why: in the JSON of the exchange format there is NO object identity — there is nothing to say "this is the same reel as above"; only nested content is expressed, and the body of a reel is written anew into each of its instances. For the owner, three reels gave 15 220 clips and 115 MB per file that the autosave writes regularly. Measurement: `.otio` against the serialization of the model — 17× on a flat project, 501× and 929× on nested scenes. The owner's decision: "we are leaving the otio format, we keep otio only for export and make breaking changes so that our format becomes a hundred times better".

#### Scenario: A second instance of a reel does not double the file
- **WHEN** a project with a reel is saved; then the same reel is placed as a second and third instance and the project is saved again
- **THEN** the growth of the file per each new instance equals the size of a REFERENCE, not of the reel's body; for a nested scene (a reel in a reel, several instances each) the file remains orders of magnitude smaller than the same project in the exchange format
- **TEST** t810 (`src/libs/kt-model/tests/project_file.rs` — growth per instance; `src/libs/kt-otio/tests/project_file.rs` — against the exchange file)

#### Scenario: The project's round trip field by field and byte by byte
- **WHEN** a project with reels, morphers, cameras, transitions, public parameters and foreign passthrough remnants is written, read and written again
- **THEN** the read model equals the original field by field, the second round is byte-for-byte, the passthrough is intact
- **TEST** t811 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The version of its own format — refusal, not a partial read
- **WHEN** a file of its own format with a version GREATER than the current one is opened; separately — a file with a lower version; separately — an exchange file `.otio`
- **THEN** the first is rejected with the version number in the error text; the second is read by migration or rejected with the name of the import item; the third is rejected with the name of the import item, rather than being read as a project
- **TEST** t812 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The file is indexable in git
- **WHEN** a project of about fifty events is saved; then one event is shifted and the project is saved again; separately — the same model is saved twice in a row; separately — a project with foreign passthrough is saved twice
- **THEN** the file is multi-line (no fewer lines than events), the line-by-line difference between two saves is in units of lines; two saves of one model are byte-for-byte equal, with passthrough too
- **TEST** t822 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: The write is honest about power loss
- **WHEN** writing a project is interrupted between the temporary file and the rename
- **THEN** the disk keeps either the previous project entirely or the new one entirely; the outcome of the write is a type, and a refusal does not look like success
- **TEST** t813 (`src/libs/kt-otio/tests/project_file.rs` — the atomic writer lives there)

### Requirement: Project file — OTIO `Timeline.1`, format detected by content
**Since R.70 this requirement describes the EXCHANGE file, not the project file** (the project — a separate requirement "The PROJECT file is its own format"). The heading is left unchanged deliberately: it is an address by which the requirement is referenced, and renaming would cost the loss of those references.

The EXCHANGE file SHALL be read and written as one OTIO `Timeline.1` with the `Stack.1` of tracks. The kind of file SHALL be determined by content (the marker of its own format, `OTIO_SCHEMA`, `format_version`), not by the extension. A root that is not a `Timeline` (for example `SerializableCollection`) SHALL be rejected with an explicit error. The legacy `.ktproj` and the exchange file `.otio` MUST NOT be opened as a project — only by a separate import item, which converts and opens the result in ITS OWN format, without touching the source.
All other requirements of this capability that describe OTIO (idempotence, passthrough, schema versions, time in ticks, media paths, model mapping, the `kt` envelope version, migrations, transitions, nested reels, generators, the legacy `.ktproj`, conformance with the reference library) SHALL refer to the IMPORT AND EXPORT DOORS, not to the project file.
Why: the owner's decision — "as a PROJECT, .ktproj is not opened anymore, no crutches are needed"; the extension cannot be trusted, because recovery and tmp files carry foreign suffixes.
R.70: the same argument is extended to `.otio` — the exchange format has ceased to be the project's format, and the door to it is now called by its own name.

#### Scenario: Detection by content and refusal on a foreign root
- **WHEN** the input is JSON of its own format, JSON with `OTIO_SCHEMA`, JSON with `format_version`, or `SerializableCollection.1`
- **THEN** the first is recognized as a project, the second — as an exchange file, the third — as a legacy project (and loading both as a project gives an error with the name of the import item), the fourth is rejected with the error "unsupported root"
- **TEST** t144, t146 (`src/libs/kt-otio/tests/format.rs`)

### Requirement: Write idempotence and a canonical key order
Writing SHALL be idempotent: `write(parse(write(x))) == write(x)` byte for byte, and at the project level `save(load(save(p))) == save(p)`. Byte-passthrough of a FOREIGN file is not promised: the first save normalizes the formatting (4 spaces, `OTIO_SCHEMA` first, known fields in declaration order, passthrough — in document order), and invariance is guaranteed for the content. All committed samples of the upstream SHALL be read and re-saved idempotently.
Why: without byte stability of the round trip, every save would "stir" the file, and a data substitution could not be told apart from reformatting; the key order requires `serde_json/preserve_order`, enabled globally for the workspace.

#### Scenario: Our own document and a foreign one after the first normalization
- **WHEN** a rich document (both tracks, gap, transition, range marker, reverse, the kt envelope) is written, read and written again; a foreign minified file goes through two rounds
- **THEN** the second output is byte-for-byte equal to the first; ticks are written as the whole number `value`, `rate: 90000.0`
- **TEST** t144 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: The round trip at the project level is stable
- **WHEN** a project with FX, bypass, reverse, volume, a link group, a region, loop and UI is saved, read and saved again
- **THEN** the model equals the original field by field, the second text equals the first; clips/media/foreign items after loading are ordered by id, so that the comparison is honest
- **TEST** t152 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Upstream samples are read and re-saved idempotently
- **WHEN** each `.otio` from `src/fixtures/otio-samples` is read, saved, read and saved again
- **THEN** none fails, the second output equals the first, there are no fewer than nine samples
- **TEST** t154 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Passthrough — "do not understand halfway"
Unknown fields of known schemas SHALL return to their original places; objects of unknown schemas and leaves newer than the supported version SHALL be stored verbatim (including the key order) and shown as a grey placeholder; foreign `metadata` namespaces MUST NOT be changed, our `kt` is appended at the end. A foreign `transition_type`, a foreign scalar of `LinearTimeWarp`, effects, markers and the track's `enabled` flag, a comment and the metadata of a marker, a disabled foreign clip SHALL return to the file untouched (with a notice where we do not play it).
Why: the owner's requirement — "do not break what we cannot read there, when saving"; a half-interpretation silently lost foreign data.

#### Scenario: Unknown fields, schemas and namespaces survive a round trip
- **WHEN** a document carries a foreign field on Timeline, a foreign namespace in metadata, a field on a known Clip and an object of the schema `FancyClip.7`
- **THEN** everything returns to its own places; the key order of the foreign object is the original
- **TEST** t145 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: A foreign transition type and time warp are not substituted
- **WHEN** the file has a `transition_type` not from our vocabulary, or a `LinearTimeWarp` with a scalar ≠ ±1
- **THEN** loading gives a notice, the transition is imported as a crossfade, and on writing the foreign type and scalar are returned as they were
- **TEST** t152 (`src/libs/kt-otio/tests/convert.rs`), t222 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Foreign content of a track, a marker and a disabled clip returns to its place
- **WHEN** a track carries foreign `effects`, `markers` and `enabled: false`; a marker has a `comment` and foreign metadata; a clip is disabled
- **THEN** all of this is in the re-saved file; the disabled track gives a notice; the disabled clip does not become our event and travels as it is
- **TEST** t384, t385, t386 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: OTIO schema versions: the single upgrade, a refusal on the spine
`Clip.1` SHALL be upgraded to `Clip.2` (`media_reference` → `media_references{DEFAULT_MEDIA}`) and written already as `Clip.2` — the only written upgrade. A leaf newer than the supported version is passthrough. The version of the BACKBONE (`Timeline`/`Stack`/`Track`) newer than the supported one SHALL give a hard refusal, checked by a preflight on the raw JSON before typing.
Why: the positions of a track's children cannot be understood halfway, and the serde bridge of polymorphic children turns a typed error into an addressless `Json(custom)`.

#### Scenario: The policy for schema versions
- **WHEN** `Clip.1`, `Clip.99`, `Track.2`, `Timeline.3` arrive
- **THEN** the first is typed and written as `Clip.2`, the second is a raw block, the third and fourth give the error `UnsupportedSchema`
- **TEST** t146 (`src/libs/kt-otio/tests/format.rs`)

### Requirement: Time in the file is only ticks; foreign rates are quantized
Our times SHALL be written as `RationalTime{value: whole number, rate: 90000}`; foreign `rate` values SHALL be quantized to ticks `round(value*90000/rate)`. The frame rate of a foreign file's project SHALL be taken by the heuristic "the most frequent rate that divides 90000", and NTSC rates (23.976/29.97/59.94) SHALL be rounded to the nearest whole number dividing 90000 if the distance is less than half a frame; otherwise the default 30 with a notice. Milliseconds MUST NOT be in the format.
Why: the 90 kHz grid does not express the NTSC series exactly (90000/23.976 = 3753.75), and a real film at 23.976 silently became a 30 fps project.

#### Scenario: NTSC rates fall onto the nearest grid
- **WHEN** `grid_fps` receives 23.976, 29.97, 59.94, 47.952, whole numbers 24…60, and also 23, 7, 0, −24, 1000, NaN, 24.4, 24.6
- **THEN** NTSC gives 24/30/60/48, whole numbers — themselves if they divide 90000, and garbage — None, "almost whole" — only within half a frame; a timeline at 23.976 is read as a 24 fps project
- **TEST** t178 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A foreign file without kt: determinism and quantization
- **WHEN** a foreign timeline at 24 fps without our envelope is imported twice
- **THEN** the models are identical, ids are assigned in traversal order, media is deduplicated by URL, the fps is picked up as 24, times are quantized to ticks, re-saving is stable, there is a notice about the default settings
- **TEST** t153 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Media paths — relative, file:// fallback, pseudo-paths verbatim
`target_url` SHALL be written relative to the project file with POSIX slashes; a path that cannot be made relative (shared only at the root) — as an absolute `file://` URL with percent-encoding; all three kinds (relative, absolute, `file://`) SHALL be read. Pseudo-paths (`synth:`, `gen:`) SHALL travel verbatim. The media path of a file ADDED to the project (dialog, drag-and-drop, command line) SHALL be made absolute relative to the current directory of the process AT THE MOMENT OF ADDITION; the model has no relative file paths, and relativity appears only on writing and only relative to the project file. A project file without a path (not yet saved) SHALL keep its paths absolute until the first write.
Why: the owner's decision "relative; fallback/absolute — only if it is in other editors" (Resolve/Hiero write absolute `file://`). R.67: in the owner's project a picture was written as `sources/a3566951817_10.jpg` relative to a file lying in `sources/` — a path relative to the launch directory got into the model as is, and on writing was taken as relative to the project; the file "disappeared" without having moved.

#### Scenario: Relativization, encoding and resolution
- **WHEN** a path nearby, deeper, higher, and on another root is relativized; a path with spaces/percent signs/UTF-8 is encoded into a URL and back; `synth:` and an already relative path pass through the writer
- **THEN** nearby/deeper/higher give relative paths, another root gives a `file://` URL; the URL decodes to the original path; `synth:` and the relative path do not change; resolution accepts all three kinds
- **TEST** t147 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: An added relative path becomes absolute
- **WHEN** media is added by the path `sources/x.jpg` with the current directory `/work` into an unsaved project; the project is saved to `/work/sources/p.otio`; it is opened again
- **THEN** in the model the path is `/work/sources/x.jpg`; in the file — `x.jpg` (relative to the project); after opening the file is found (in the previous code — `sources/x.jpg` relative to the project, that is `/work/sources/sources/x.jpg`, the file was lost)
- **TEST** t797 (`src/apps/kt-app/src/layout_tests.rs`), t797b (`src/libs/kt-otio/tests/format.rs`)

### Requirement: The entry point is counted from the first frame of the file
`source_range` and `available_range` are one coordinate system of the material; in broadcast files it starts from the embedded timecode. The start of `available_range` SHALL be subtracted on reading and returned on writing; a clip to the left of the declared start of the material is clamped to zero.
A clip without `source_range` SHALL take the material whole with a ZERO entry point; a clip with `media_reference: null` SHALL be read as a clip without material, receive a separate media record and be written back with `null`, rather than an invented reference. The references to material (`available_range`, variants) are a property of the MEDIA: an event born of an edit SHALL be written in the same coordinates as its neighbour from the file, while the clip's own envelope (a foreign `active_key`, the set of variants) MUST NOT be overwritten.
Why: without the subtraction the engine drifted an hour into a 162-second file; without a media level the two halves of one cut drifted into the file in different coordinate systems.

#### Scenario: The timecode is subtracted and returned
- **WHEN** a clip with `source_range.start` = an hour + 10 s, with `available_range.start` = an hour; the same file without a timecode; a clip to the left of the start of the material
- **THEN** `source_in` = 10 s from the start of the file; on writing the timecode returned; the file without a timecode did not change; to the left of the material — zero
- **TEST** t180 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A clip without `source_range` and without a reference
- **WHEN** `simple_cut.otio` (Clip-004 without `source_range`) and `transition_test.otio` (all references `null`) are read
- **THEN** the duration is taken from `available_range`, the entry point is zero; clips without a reference get an empty path and different media ids, `null` survives re-saving
- **TEST** t154 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A newborn event in the coordinates of the material
- **WHEN** a clip with an hour-long timecode is cut in half and the project is saved; a clip with two reference variants and `active_key: proxy` is re-saved
- **THEN** both halves in the file lie in the coordinates of the material with `available_range`; the active key and both references are intact
- **TEST** t377, t378 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

### Requirement: Mapping the model to OTIO — one code and known conventions
Video tracks SHALL be written inverted (the first video track of the file is the lower layer), audio — after the video in direct order. FX SHALL be written as the standard `Effect.1` with `effect_name = "kt:<kind>"`, `enabled = !bypass` and the full instance in `metadata.kt`; reverse — `LinearTimeWarp{−1}` FIRST in the chain. The default track labels (V1/A1) MUST NOT be written to `Track.name`. Parameter values SHALL survive a round trip without clamping by the reader (in particular a signed and zero speed). Overlapping clips of one track SHALL be written as neighbouring `Track`s according to the `metadata.kt.layers` map; without overlaps the map is not written. The track number on reading SHALL become the rank of its row, and a track's foreign raw data — travels with its own track.
Why: in OTIO the clip's position is the order of the track's children, and overlap within one track is physically inexpressible (verified on the owner's file); the owner's file with a track "under the ceiling" hid an event in the lower layer.

#### Scenario: Inversion of video tracks and standard effects
- **WHEN** a project with two video tracks and an audio pair with a transition is written
- **THEN** the first video track of the file is the bottom track, the top one is the last video, then audio; on reading the clips equal field by field, the marker colour is normalized to GREEN
- **TEST** t152 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An overlapping track is split into tracks and assembled back
- **WHEN** three clips of one video track are saved, two of which overlap
- **THEN** the file has two `Track`s; on reading all three clips are again on track 0 with the same geometry; a project without overlaps has no `layers` key
- **TEST** t165 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The owner's file is read canonically
- **WHEN** the row order puts a video track with a larger number above tracks with smaller numbers, and its track carries foreign metadata
- **THEN** after reading the numbers equal the ranks of the rows, the order on screen is the same, the foreign raw data lies on the track of the same event; the round trip is idempotent
- **TEST** t593 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A signed speed survives a round trip
- **WHEN** slowed&reverb with base −0.5 and keys 1 → 0 → −2 is saved and read
- **THEN** the base and keys equal the originals; zero and minus are not pulled up to the former minimum
- **TEST** t606 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Version of the `kt` envelope — one per file, set BY CONTENT
`Timeline.metadata.kt.v` SHALL be the single version of the file; nested envelopes carry no versions. The writer SHALL set the OLDER version at which an old build would read the file without silent corruption: v2 — cameras; v3 — curves in the ticks of the material, switches, the kind of transition and the audio curve; v4 — the morpher layer and areas; v5 — curves and the strength of a layer; v6 — nested reels; v7 — inversion and overlapping transitions; v8 — an event to the left of zero in a reel; v9 — the generator on the event, public parameters and overrides; **v10 — the shape of a segment on a key, working sources of a curve (the link to an address and an expression, a formula with text) and the default shape of the project** (one version per round, a single predicate).
A file newer than the supported one SHALL be rejected with `KtMetaTooNew`; unknown fields of our envelope SHALL pass through as passthrough. A file without novelties MUST remain byte-stable and on the previous version — the cascade is checked over all committed fixtures.
**BREAKING (model):** the shape moves from the curve to the KEY, the field `ParamCurve::ease` in the model is removed. In the FILE the shape is written BY CONTENT, by the same rule as the version: a curve whose keys all carry one shape SHALL be written in the former field `ease` (third order — `true`, straight — `false`) and does not move the version; a curve with different shapes, or with a shape not expressible by this field, SHALL write the shape on the keys and raise the file to v10. On reading: `ease` expands into a uniform shape; the shape on a key overrides it. A link to a vanished property and an unparseable formula SHALL be rejected on reading, with an address and a reason.
Why: the owner does not require backward compatibility, but "ours so that nobody breaks it": a refusal is more honest than silent corruption, and a condition built into the cascade in the wrong place raises the version for all files in a row; a silent change of shape during migration would shift the frames of finished projects.

#### Scenario: A file newer than the build is rejected, the version is read
- **WHEN** the envelope has `v: 999`; in an ordinary file `v: 1`
- **THEN** the version check gives `KtMetaTooNew(999)`; the version is read as 1
- **TEST** t146 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: Cameras — v2, automation — v3, without them — v1; future fields pass through
- **WHEN** a project without cameras, a project with cameras without curves, and a project with cameras and curves are written; an unknown field is inserted into the envelope
- **THEN** versions 1/2/3 respectively; without cameras there are no camera keys; the round trip is idempotent; the unknown field returns; the output is byte-for-byte equal to the golden `demo-3d.otio`
- **TEST** t189, t189b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Effect automation raises to v3, a broken curve is a refusal
- **WHEN** a flat project with a blur radius curve is saved; then in the file a key with a negative position; then `auto` is replaced with garbage
- **THEN** v3, the round trip field by field and byte by byte; both spoiled files fail the load explicitly
- **TEST** t192 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The morpher layer and areas — v4, the natural order of rows is not stored
- **WHEN** a morpher becomes a "by time" layer over everything below; a named area is added to the project; the row order is shuffled
- **THEN** the file is v4, the layer's defaults are not serialized; the area survives the round trip; the order is written in its own field `lane_order` outside `ui`; a project without morphers/areas/reorderings carries none of these keys and does not move the version
- **TEST** t240, t241, t248 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The version cascade does not move old files
- **WHEN** each committed fixture `src/fixtures/*.otio` is read and re-saved
- **THEN** the envelope version is the same as it was, the output is byte-for-byte equal to the source, there are no attachments in them
- **TEST** t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Public parameters and overrides — v9
- **WHEN** a reel declares a handle "subtitle text", the second instance carries its own value; an ordinary reel without publications
- **THEN** the first file is v9, the set and the overrides survive the round trip idempotently; the second contains neither `published` nor `overrides` and remains v6
- **TEST** t681 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A shape, a link and a formula raise to v10
- **WHEN** a project without new capabilities is saved; then a project with a segment shape; then with a link; then with a formula
- **THEN** the first remains the previous version and byte-for-byte as before, the rest get v10; the version in the file is one
- **TEST** t698b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The v10 round trip and the uniform shape in the former field
- **WHEN** a v10 file with different shapes on keys is read and written; then a file with the old `ease` field in both values is read; then a curve whose keys all carry one shape is written
- **THEN** the round trip is identical; `ease = true` became the third order with the former smoothing, `ease = false` — straight, the values of the curve after reading are bitwise equal to the values before; the uniform curve is written in the former field `ease`, the bytes and the file version did not move
- **TEST** t698c (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An unresolvable source in the file — an error with an address
- **WHEN** a file is read where a link points to a vanished event and a formula does not parse
- **THEN** the reading is rejected with an address and a reason, rather than silently drawing the curve from its keys
- **TEST** t698d (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Old files migrate on read, one-way
Files of versions up to the current one SHALL be read and translated into the current semantics on reading; the next write SHALL already go in the new form: curves in fractions 0..1 → material ticks by the geometry of the clip (v<3); camera keyframes in fractions → common curves; a morpher before v4 → a patch with `timed: false`; `ui.lane_order` → the model's `lane_order`; generator values in `parameters` and the animation mode of a title → values on the event and curves by the same formulas as the preset in the window; the former boolean `ease` of a curve → the shape of a segment on the key, and for a SPEED curve (`rate`) `ease: true` SHALL be read as the STRAIGHT shape — that is how it always sounded (the warp map rests on a piecewise-linear profile and never read the `ease` field). The migration MUST NOT produce a file that its own validation rejects: a file that was opened by the previous build SHALL open with the next one too, and the sound and the frame SHALL remain as before.
Why: backward compatibility is not required, but the owner's working projects must open and sound/look as before; a divergence of the migration from the preset would give "the same button — a different result". R.63: `untitled_v26.09.19.otio` (14 speed curves with `ease: true`) was rejected by `RateShapeMustBeLinear` — the reader translated `ease` as "smooth", while the speed validator accepts only the straight one; the previous warp did not see the shape, so the correct translation is "linear".

#### Scenario: Curves of the old domain are translated into material ticks
- **WHEN** a v2 file with keys 0.0 and 0.5 on a clip 0..90000 is read and re-saved
- **THEN** the keys became 0 and 45000; the write is v3
- **TEST** t212 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Legacy camera keyframes, the old morpher and the old place of order
- **WHEN** a camera carries `keyframes` in fractions of the event; a morpher was written as a bare array of targets and an externally tagged kind; the row order lies in `ui.lane_order`
- **THEN** the keyframes are eaten by the migration into curves in ticks; the morpher is read with `timed: false` and the previous semantics, it does not raise the version to v4 or above; the order moves into the model
- **TEST** t189, t240, t241 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The generator of an old file hands the values to the event
- **WHEN** a v8 file with text and an animation mode inside `parameters` and without `kt.gen` on the clip
- **THEN** the media path is shortened to its kind, the values moved to the event, the animation curves equal those set by the preset, the project is valid
- **TEST** t665 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A speed curve with the former `ease` is read as straight and the file opens
- **WHEN** a file before R.61 is read with `rate` curves `ease: true` on an event, in a morpher's patch and inside a nested reel (the file shape — as in `untitled_v26.09.19.otio`), and the same with `ease: false`
- **THEN** the project is valid, all speed keys have the shape `Linear`, the other curves with `ease: true` give the previous smoothing; the warp map gives the same ticks as before R.61; re-saving writes `shape` only where it is not the default; on the previous code — the refusal `RateShapeMustBeLinear`
- **TEST** t747 (`src/libs/kt-otio/tests/convert.rs`), t747b (`src/libs/kt-model/tests/fx_auto.rs`), live scenario `src/scripts/live/oldfile.py`

### Requirement: Our own broken section — an error with an address; a broken record — a notice
The absence of a `kt` section SHALL mean an ordinary foreign file. A `kt` section that is present but cannot be read (in Timeline, in a clip, in an attachment) SHALL fail the load with the error `KtMetaBroken` and a list of the addresses of all broken clips (name, kind, track, position); the source file is not touched.
An unreadable SEPARATE record (a kind of transition, an overlap transition with an unknown clip) SHALL give a notice and an honest default, not silence.
Why: a silently discarded envelope gave the clip a new number and irreversibly broke the A/V link groups and the morpher references; on a live scenario one wrong byte opened the project "empty".

#### Scenario: A broken envelope of Timeline and of a clip
- **WHEN** in the Timeline envelope one field is not a number; a clip has a `kt` section with `id: "not a number"`; a clip has no section, or has only a foreign namespace
- **THEN** the first two — `KtMetaBroken`, the message about the clip carries its name and track; the last two are read
- **TEST** t251, t383 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A broken kind of transition and a broken record of an overlap transition
- **WHEN** the `kind` of a transition is "nonsense"; an overlap transition's `b` points to a nonexistent clip
- **THEN** the file opens, the transition is a crossfade with a notice; the overlap transition is skipped with a notice
- **TEST** t222, t465 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Transitions in the file — a standard type outward, the kind inside the envelope
A transition SHALL be written as `Transition.1` between neighbours with a meaningful `transition_type` (`SMPTE_Dissolve` / `SMPTE_Wipe` / `Custom`); a kind with parameters, the audio curve, the curves of parameters and the inversion SHALL travel in `Transition.metadata.kt`. An ordinary crossfade with equal-power MUST NOT have an envelope written; a return to a crossfade SHALL clear the envelope entirely.
A transition as the first/last child of a track or next to a `Gap` SHALL be read as a fade (a transition with an empty side) and written back to its own place. An overlap transition (a pair of neighbouring `Track`s of the layer map) SHALL travel as a list `overlap_transitions` in the envelope of its own reel, without offsets.
Why: an old build would have played a wipe as a dissolve; an obsolete `"kind":"wipe"` from the retained data would have gone back into the file; an overlapping pair cannot be expressed as a child of a sequential track.

#### Scenario: Wipe, curves, inversion and cleaning of the envelope
- **WHEN** a transition is a crossfade; then a wipe with an S-curve; then a return to a crossfade; then an angle curve; then inversion and its switching off
- **THEN** the crossfade is v1 without an envelope; the wipe is v3, `SMPTE_Wipe`, the kind and the curve are in the envelope; the return is v1, the envelope is removed; the curves are v3 and the round trip; the inversion is v7, switching off removes the key and the version
- **TEST** t222 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An overlap transition in the envelope of a reel
- **WHEN** two video clips of one track in a staircase with a Barn transition, an S-curve and inversion
- **THEN** the file is v7, the record `{a, b, kind, audio, invert}` in `overlap_transitions`; after reading the transition window equals the overlap zone; the round trip is idempotent
- **TEST** t465 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Nested reels — `Stack` as the child of `Track`, body inline, sidecar advisory
In the EXCHANGE file an instance of a reel SHALL be written as a `Stack` child of a `Track` (the window — the `source_range` of the stack, the name — `Stack.name`, the chain and reverse — `Stack.effects`, the address of the reel — `metadata.kt.seq`), and the body of the reel SHALL be inlined into EACH instance. On reading, the instances SHALL collapse into one reel by address; diverged bodies SHALL become separate reels with fresh ids and a notice. The sidecar `metadata.kt.sequences` (tracks, the order of rows, morphers, cameras, the layer map of the reel) SHALL be advisory: if it is lost, the edit is read from the file with a notice. Foreign nested `Stack` and `Track` inside a `Track` SHALL be read as genuine reels. An event to the left of the reel's zero SHALL carry `kt.start` only where the position in the track does not express it (v8).
In the PROJECT file the body of a reel SHALL be stored ONCE, and the instance SHALL be a reference to its address: the inline is not needed there and is forbidden at a price — it is what gave 115 MB.
Why: there is no object identity in the JSON of OTIO; a silent choice of one of two bodies is a loss of someone else's work; a foreign editor draws the content of attachments, and a grey placeholder does not.
R.70: the same fact (no object identity) means that for the PROJECT the inline is not a merit but a multiplicative price; the requirement is divided by addressee.

#### Scenario: A reel in two instances survives a round trip
- **WHEN** a reel with a crossfade, a morpher layer and a camera stands twice, the instance has level 0.5 and reverse; the file is an EXPORT to OTIO
- **THEN** the file has two `Stack`s with non-empty bodies, version v6; after reading one reel, the content field by field, the instances field by field, the second round is byte-for-byte
- **TEST** t431 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Diverged bodies and a lost sidecar
- **WHEN** the body of the second instance is changed by a foreign editor; the `sequences` sidecar is deleted
- **THEN** the first gives two reels with a notice and a valid project; the second — the reel with the edit and the crossfade from the file, without the morpher, with a notice
- **TEST** t432, t433 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Foreign attachments become genuine reels
- **WHEN** `nested_example.otio` is read (two `Stack`s and a `Track` inside a `Track`)
- **THEN** three reels with events, foreign is empty; after unfolding, no event points at a reel; the round trip is idempotent
- **TEST** t437 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An event to the left of zero inside a reel
- **WHEN** the first event of a reel starts at −10000, the second stands at its own time
- **THEN** `kt.start` is present on exactly two shifted events, the file is v8, the round trip is byte-for-byte; an ordinary edit carries no such field
- **TEST** t638 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Generators — `GeneratorReference.1`; a foreign kind — passthrough and a placeholder
Our generator SHALL be written as `GeneratorReference.1` with `generator_kind = "kt.<kind>"`, the values of the event in `parameters` and a MANDATORY `available_range` (0..source_in+duration, rate 90000); only the kind remains in the media path. A foreign `generator_kind` (not `kt.*` and not a known alias) MUST NOT be typed: a noticeable stub (an empty path) plus passthrough of the node. The upstream alias `SMPTEBars` SHALL be read by our bars generator and canonicalized on re-saving. A generator on the event SHALL raise the file to v9.
Why: a generator has no duration, but the format requires naming one; the byte-idempotence of foreign files costs more than typing; the values moved from the media path to the event, and an old build would have looked for them where they no longer are.

#### Scenario: Our generator at the root and inside a reel
- **WHEN** a solid generator stands at the root and inside a reel, next to a `synth:` source
- **THEN** both are `GeneratorReference.1` with `kt.solid`, parameters and `available_range` from zero; `synth:` is `ExternalReference` verbatim; the media path is `gen:solid`; the file is v9, a project without a generator does not raise the version; the round trip is idempotent
- **TEST** t473 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A foreign kind and an alias
- **WHEN** `generator_kind` is replaced by `Legacy.Title`; and by `SMPTEBars`
- **THEN** the first is read as a stub with an empty path and re-saved with its kind and parameters; the second is read as `gen:smpte` and canonicalized to `kt.smpte`; both round trips are idempotent
- **TEST** t474, t494 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Two titles of one kind with different text
- **WHEN** two events of one kind `gen:text` carry different text and animation curves
- **THEN** the file is v9, each has its own `GeneratorReference` with the event's text, the media path remains `gen:text`; the round trip is field by field and byte-for-byte
- **TEST** t664 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: A derived event inherits its parent's envelope; the writer lays out markers
An event born of an edit (the half of a cut, a piece of a bake, a paste) SHALL receive the envelope of its parent entirely — the name, foreign effects, foreign metadata, markers; the writer SHALL distribute the markers by their intersection with the material window of each event.
Why: distributing markers on a cut would mean losing them under undo; nameless halves without foreign data were a loss for a foreign editor.

#### Scenario: Name, effects, metadata and markers after a cut
- **WHEN** a clip with a name, a foreign effect, foreign metadata and three markers is cut in half
- **THEN** both halves have the same name, effect and metadata; the markers are distributed: "early" — to the first, "boundary" and "late" — to the second
- **TEST** t380, t381 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

### Requirement: Legacy `.ktproj` — import only; the golden freezes the conversion and schema
The import of `.ktproj` SHALL convert overlaps by the rules: with layering switched off, an audio pair → a trim to the middle of the overlap plus a transition in the same window; 3+ and FX overlaps → a split across tracks without volume compensation; a video overlap → demotion of the later one below; with layering switched on (the default) the import SHALL change nothing except normalization.
The result of importing `fixtures/demo.ktproj` SHALL be byte-for-byte equal to the committed `fixtures/demo-legacy-import.otio`.
Why: the owner's decisions — "a split without volume compensation" and "as it was, with overlaps" (his project of 1952 clips was unfolding into ~40 tracks); the golden catches the drift of the serde schemas of reusable model types.

#### Scenario: Conversion of overlaps with layering switched off
- **WHEN** an audio pair with an overlap, a reversed incoming clip, a triple overlap, a pair with FX, a video overlap
- **THEN** the pair becomes a transition with the window of the overlap; reverse does not move the entry point; the triple and the FX are split across tracks; the late video clip goes to the track below
- **TEST** t143 (`src/libs/kt-model/tests/legacy_upgrade.rs`)

#### Scenario: Import "as it was" with layering
- **WHEN** a mix of an A/V pair with an overlap, a triple zone and a micro-overlap of 1 tick is imported with layering switched on
- **THEN** no clip is touched, the tracks have not multiplied
- **TEST** t164 (`src/libs/kt-model/tests/legacy_upgrade.rs`)

#### Scenario: Golden of the legacy import
- **WHEN** `demo.ktproj` is imported and saved
- **THEN** the output is byte-for-byte equal to `demo-legacy-import.otio`
- **TEST** t148 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Conformance with the reference OpenTimelineIO library
Our file SHALL survive a round trip through the reference Python library (reading and rewriting by the reference → our reader restores an equivalent model; the reference is obliged to keep `metadata.kt`), and a timeline built with the reference's API SHALL be read by us and, after our re-saving, checked by the reference's assertions. `OTIO_SCHEMA` SHALL be on EVERY serializable object, including the nested `RationalTime`/`TimeRange` and the `Stack` in `Timeline.tracks`. The test is gated: without `opentimelineio` it is skipped with a reason; `KT_REQUIRE_OTIO_CONFORMANCE=1` requires it.
Why: the format has no official JSON Schema, the norm is the implementation; our idempotence tests forgive our own "incomplete", and only the reference found bare dictionaries without a tag.

#### Scenario: Two round trips through the reference
- **WHEN** `python3 -c "import opentimelineio"` is available
- **THEN** our file after rewriting by the reference is read into an equivalent model; a timeline built by the reference with a transition and a marker is read, and our re-save passes the Python assertions
- **TEST** t149 (`src/libs/kt-otio/tests/otio_conformance.rs`)

### Requirement: Atomic write is honest to power loss
The project write SHALL go through one place: a temporary file with the full name plus `.<pid>.tmp` → writing and `fsync` of the file → `rename` → `fsync` of the directory. A failure of the DIRECTORY sync MUST NOT count as a write failure: the file is in place and intact, the degradation is reported by a flag.
Why: a rename that reached the disk before the contents leaves an empty file in place of a whole one; on exFAT and network shares the directory sync is legitimately unsupported; two copies of the program with one tmp name would replace the project with half-written contents.

#### Scenario: The order of operations and the degradation of the directory
- **WHEN** the write goes through an injected file system with a journal; then the directory sync answers `Unsupported`
- **THEN** the journal is exactly "write+sync `a.b.otio.<pid>.tmp` → rename → sync dir", no temporary file remains; in the second case the write succeeds, `dir_synced == false`, the contents are in place
- **TEST** t351, t353 (`src/libs/kt-otio/tests/io.rs`)

### Requirement: Conform rule, leading node and reel settings in the file — v11 by content
The file SHALL raise the envelope version to v11 only when it carries at least one of: a project conform rule other than "fit"; a leading "Geometry" node with a rule other than "follow the project", or with non-identity parameters; the reel's own raster or fps; a flag of explicit project settings. A leading node with defaults MUST NOT be written — an event without a node in the file receives it on reading. An old `Effect` of the kind `kt:Pan` SHALL be read into the event's leading node (mode → rule, shift and zoom → position and zoom) one-way and never written again. One version per round of R.62: the change `track-object-and-track-fx` extends the predicate of v11 with its own fields.
Why: a file without novelties must remain byte-stable on the previous version (the cascade over the fixtures), and migration on reading is the only legitimate path for old files.

#### Scenario: The round trip of v11 and the byte-stability of the old
- **WHEN** a project without novelties is saved; a project with an explicit rule of the event; a project with the project rule "fill"; a reel with its own raster; then a file with the old `kt:Pan` is read
- **THEN** the first — the previous version and byte-for-byte (t433b); the rest — v11 with an idempotent round trip; `Pan` is read into the leading node with the same numbers and does not appear on writing
- **TEST** t728 (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Track in the file — track envelope, `enabled`, track effects, counter migration
`Track.metadata.kt` SHALL carry the id, the switches, the level, the blend mode and the track height (defaults are not written); `Track.enabled` SHALL equal `!mute` and be read back as mute (a foreign disabled track becomes muted, not enabled); the track's chain SHALL be written as standard `Effect.1` in `Track.effects` by the same code as for a clip; the name — in `Track.name`, default labels are still not written; there is no separate field for the order of rows — the order of the file's tracks. Old files with counters, `lane_order` and `ui.lane_names/lane_heights` SHALL be migrated on reading into tracks one-way. The v11 predicate of the R.62 round SHALL count any non-default track property as a novelty; a file without novelties is byte-stable on the previous version.
Why: neighbours have track effects and `enabled`, and the round trip was losing them (`TrackExtra`); the notice "Kinotrain has no per-track mute" is removed in substance.

#### Scenario: Track round trip, foreign `enabled`, migration of an old file
- **WHEN** a project with a track's mute/solo/lock/level/mode/chain is written and read; a foreign file with `enabled: false` on a track is read and re-saved; an old file with `lane_order` and `ui.lane_names` is read
- **THEN** the round trip is idempotent and v11; the foreign track became mute and is written with `enabled: false`; the old file yielded tracks with the same names, heights and order; a file without the new features is byte-stable (t433b)
- **TEST** t731 (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Compressor key input in the file — v12 by content
The sidechain key SHALL be written in the compressor's parameters as the TRACK address (`TrackId`), not its number: the number is the row rank, and inserting a track above would move the key onto a foreign signal. A compressor WITHOUT a key MUST NOT write the field at all: a project file without a sidechain stays byte-for-byte the same and on the same envelope version. A file carrying at least one assigned key SHALL raise the envelope version to v12: an old build would lose the key silently and sound different, and the version is the only honest way to tell it so.
Reading SHALL be tolerant: a key pointing at a track that is not in the file is read as "no key" (the device is a plain compressor), the project opens, there is no error. The "read → write" round trip with a key SHALL be idempotent.
Why: the envelope rule is a version by CONTENT and one per round; an address by row rank already caused state to move onto a neighbour (R.62, the `TrackId` key in FX state).

#### Scenario: v12 round trip and byte stability of a file without a sidechain
- **WHEN** a project with a compressor without a key is saved; then the same project with an assigned key is saved; then a file is read where the key points to a deleted track
- **THEN** the first is the prior version and byte-for-byte; the second is v12 with an idempotent round trip and the same `TrackId` after reading; the third opens, the key is read as absent, and writing does not resurrect it
- **TEST** t773 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Import of an old-version project — a separate door; fixes lapse with versions
A envelope file of a version lower than the current one, which the ordinary reader rejects, SHALL be opened via an IMPORT: a separate entry point that applies REPAIRS of known old forms to the document before the ordinary read, and returns the project in the current form plus the list of repairs made (as notices). The reader MUST NOT learn new old forms: a repair goes into the import, each one tagged with the version it was made for, and is DELETED when support for that version is dropped (the list of versions-with-repairs is one place). Ordinary opening of such a file SHALL be rejected with an error naming the file's version and pointing to the import. The first repair: `targets: null` on a morpher of v5 ⇒ "everything below the row". The import does not write the source file; the next save goes out in the current form and version.
Why: the owner's decision (R.67) — "fix it in the import… and pin it for other agents too, so that they automatically put fixes for old versions into the import; these fixes will gradually be deleted as old versions are retired"; a reader with accumulated permanent migrations is exactly the "backward compatibility" that "is not needed".

#### Scenario: Import repairs v5, the reader is untouched
- **WHEN** a v5 file with `targets: null` on three morphers, an object `{on: clips}` on two, and a list on one is read by the ordinary reader; then by the import; then the imported project is saved and read by the reader
- **THEN** the reader rejects it with version 5 and the word "import"; the import yields a project with six morphers: three "everything below", two by list, one by list; in the notices — "morpher targets: null → below (v5)" ×3; the written file is read by the reader without repairs (the version is by content, no `null` remains in it); the repair list in the code carries a version for each
- **TEST** t783b (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Segment formula and formula layer in the file — v13 by content
A key with the "segment formula" form and a curve with a formula layer SHALL be written to the curve envelope; the envelope version SHALL be raised to 13 ONLY when the file contains at least one of them; a file without them SHALL be written with the same bytes and the same version. An old build reading such a file SHALL refuse it by version (a segment form it does not know would sound like a straight line, and a layer would be silently ignored), not read it partially.
Why: the rule "the version is one and is raised by content" (v9–v12); a silent reading of an unknown form is a corrupted edit without any sign.

#### Scenario: Round trip to v13 by content
- **WHEN** a project with a segment formula is saved and read; a project with a layer is the same; a project without them is saved after a v12 file was opened
- **THEN** the first two are v13, the round trip is byte-for-byte, the forms and the layer are the same; the third is v12 with the prior bytes; a v12 reader refuses a v13 file by version
- **TEST** t787b, t787d (`src/libs/kt-otio/tests/convert.rs`)

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
