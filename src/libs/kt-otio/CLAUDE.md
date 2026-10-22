# kt-otio

> The crate's contracts and mandatory tests — `openspec/specs/{project-format}/spec.md`; the reasons for the decisions — `design.md` next to it.

**Since R.70 OpenTimelineIO is an EXCHANGE format, not a project format.** The project file is its own
`.kt` (`kt_model::project_file`), and this crate is two named doors
(`import_interchange` ← `File → Import older Kinetren project…`,
`save_project` ← `File → Export → OpenTimelineIO…`), the detection of the kind of file by
content (`detect`: own / OTIO / legacy) and the single home of the atomic
write (`write_atomic`). Everything below about idempotence, passthrough and envelope versions is about the DOORS, and remains in force. Pure Rust (serde_json), WITHOUT
FFmpeg/audio/C++ bindings/url crate. Dependencies: kt-time (+kt-model from the
convert stage). The reference of the semantics is the C++/Python library OTIO 0.18.x.

## Invariants

- **Idempotence of the write** (test 144): `write(parse(write(x))) == write(x)`
  byte-for-byte. A byte passthrough of a FOREIGN file is not promised — the first save
  normalizes the formatting (the reference does this too); the invariance is guaranteed
  for the content, not for the whitespace.
- **Passthrough, "do not understand by halves"** (tests 145-146): unknown FIELDS of
  known schemas — in `#[serde(flatten)] extra` at the original places; objects
  of unknown schemas/leaf versions — `RawObject` verbatim; foreign
  metadata namespaces are untouched, our `"kt"` is appended at the end. The version
  of the SPINE (Timeline/Stack/Track) newer than the supported one → a hard refusal
  (the positions of the children would become a guess) — it is checked by a preflight on the raw Value:
  the serde bridge turns typed errors into `Json(custom)`, therefore
  the refusal of the spine happens BEFORE the `typed()` parse, not inside it.
- **Canonical order of keys**: `OTIO_SCHEMA` first, the known fields in
  the order of declaration of the struct (frozen by tests), `extra`/metadata — the order
  of the document. It requires `serde_json/preserve_order` — the feature is GLOBAL for the
  workspace (cargo unification): `serde_json::Map` everywhere becomes an IndexMap.
  The audit at the moment of enabling: Value lived only in the tests of kt-model — safe;
  new Value consumers MUST not rely on alphabetical order.
- **The starting timecode of the material (item 180)**: `source_range` and
  `available_range` are one coordinate system; for broadcast files it starts from the burnt-in
  timecode. The model's `source_in` is counted FROM THE FIRST
  FRAME of the file, therefore the start of `available_range` is subtracted on reading and
  added back on writing. A clip without `source_range` takes the material whole, with a
  ZERO entry point.
- **NTSC rates (item 178)**: 23.976/29.97/59.94 do not divide the 90 kHz grid
  (90000/23.976 = 3753.75). `grid_fps` rounds them to the nearest integer
  that divides 90000 (otherwise such a file would silently become a 30 fps project).
  The splices do not suffer from this — they are in ticks; the byte-stability of the round trip
  does not hold on such files (a tick per clip).
- **Time**: `RationalTime{value, rate}` — our ticks are written as value=integer
  (double precision is exact up to 2^53), rate=90000.0; foreign rates are quantized by
  `round(value*90000/rate)` — the policy of the entire system.
- **`OTIO_SCHEMA` on EVERY SerializableObject** — the reference rejects bare
  dictionaries, including nested RationalTime/TimeRange (manual Serialize with a tag,
  duration before start_time — the order of the reference) and Stack in `Timeline.tracks`
  (the field goes past the Composable bridge → serialize_with/deserialize_with; on
  reading the tag is stripped, so as not to settle in extra). Our idempotence tests
  do NOT catch this (they forgive their own "incomplete") — only the conformance test t.149 catches it.
- `metadata.kt.v` — one version per FILE (Timeline); newer than ours → an explicit refusal
  `KtMetaTooNew`. Nested kt-envelopes do not carry versions.
- Upgrade of schemas: only Clip.1→2 (media_reference → media_references), as in
  the reference. Marker.1 and other archaic schemas — a passthrough list.
- `detect()` — by CONTENT (OTIO_SCHEMA against format_version); we do not trust the extension.
  .ktproj does not open as a project (R.24 item 12) — only as an import.
- Paths (test 147): target_url is written relative (POSIX slashes), the fallback —
  a file:// URL with percent-encoding (as Resolve/Hiero do); relative /
  absolute / file:// are read; `synth:` — verbatim.

- **Foreign things that the format permits** (item 154): a clip without `source_range` takes
  the whole `available_range`; `media_reference: null` — a legitimate clip without material
  (null is carried through the round trip, an ExternalReference is not invented);
  `name: null` is normalized to an empty string (as in the reference).
- **A tail Gap = the editing length** (item 158): it is read into
  `Project.timeline_end`, on writing it is appended to each track shorter than the end. An empty track that was not in the file is NOT written — the number of our tracks
  lives in `metadata.kt.lanes`.
- **An edge transition is a fade, not garbage** (item 155): a transition as the first/last
  child of a track and a transition next to a Gap are imported as a `TransitionEvent`
  with an empty side and are written back to their own place.

- **Layers (R.28, item 165)**: overlapped clips of one track are written as adjacent
  `Track`s (in OTIO the position of a clip is the order of the children; an overlay in one track
  is inexpressible), the map `metadata.kt.layers` holds the number of tracks per track and
  reassembles them. If it does not match the file — it is ignored with a notice. Layer 0
  carries the track's name and the foreign extras/gaps. Without overlaps the map is not written, and
  the file is byte-for-byte as before.

- **Broken — refusal, foreign — passthrough (R.49, item 358)**: OUR clip section is
  read by `kt_get_checked`, as for Timeline (t251). "There is no section" — an ordinary
  foreign file; "the section is there and broken" — an error with a list of the addresses of ALL the broken
  clips. Silent dropping gave a clip a new number and irreversibly broke the A/V link groups
  and the morpher references. The foreign items of a track (`effects`, `markers`,
  `enabled`) and of a marker (`comment`, `metadata`, `extra` —
  `Retained.stack_marker_extras`) are returned to their place; a disabled foreign
  track gives a notice. `Clip.enabled` is not repaired — it is already verbatim through
  ForeignEvent. Tests 383-386.

- **References to the material are a property of the MEDIA (R.49, item 356)**: `Retained.media`
  (`HashMap<MediaId, MediaRefs>`) is filled in on reading and serves as the fallback
  path on writing for clips BORN in the editor (a half of a cut, a piece of a bake, an insertion, an event from the bin) — they do not have their own `ClipExtra`. Without this
  the two halves of one cut went into the file in different coordinate systems:
  the left from the burnt-in timecode, the right from zero. A clip's own envelope does NOT overwrite the media level
  (a foreign `active_key` and the set of variants are intact). Tests 377-378,
  `tests/edit_roundtrip.rs` — the round trip "loaded → edited → saved →
  loaded", the only one in the crate where an edit happens at all.

- **The level of an event (R.28, item 174)**: `metadata.kt.opacity` of a clip; 1.0 is not
  written — files without a level are byte-stable.

- **Cameras and the software version by CONTENT (R.29, item 189)**: `KT_META_VERSION = 2`,
  but the writer sets v=2 only when the file carries cameras/camera tracks
  (`metadata.kt.cameras`/`camera_lanes`, sorted by CameraId) — an old reader would silently lose the 3D scene on overwriting. Files without cameras — v1
  byte-for-byte (t144/t148 were not regenerated). A Pan3d alone does not bump
  the version (foreign passthrough in an old reader). Unknown fields of OUR
  envelope (kt) — flatten-extra in KtTimelineMeta/KtClipMeta, passthrough
  via Retained (timeline_kt_extra / ClipExtra.kt_extra): additions of v3
  will not need a hard refusal. The automation of FX (R.30) — the key `auto` inside the kt-envelope of the
  effect; its presence also raises the file to v2. The golden v2 schema —
  fixtures/demo-3d.otio (t189_regen; it freezes both the cameras and the curves).

- **The kind of transition (R.33, item 222)**: `kind` and `audio` in `Transition.metadata.kt`;
  outwards — a meaningful `transition_type` (SMPTE_Dissolve/SMPTE_Wipe/Custom),
  a FOREIGN type from Retained is not substituted. A non-crossfade or a non-default curve
  raises the file to v3 (an old build would play a wipe as a dissolve). The return
  to a crossfade CLEANS our envelope out of the retained metadata — otherwise the outdated
  kind would travel back into the file; an emptied `kt` is removed entirely, so that the foreign
  file stays byte-stable. Test 222.

## v1 limitations (documented, not implemented)

A SerializableCollection root → an error; .otioz/.otiod — no; the Retained binding
of tracks/transitions by indices may re-glue foreign metadata after
structural edits (clips are not affected — the key is ClipId).

## Tests

144 (round trip + idempotence), 145 (passthrough), 146 (versions/upgrade/kt.v),
222 (kind of transition, audio curve, conditional v3, envelope cleanup),
147 (paths/URL), 148 golden of the legacy import, 149 conformance with the python
opentimelineio (gated), 152-153 mapping/foreign import, 154 foreign samples
of upstream (`fixtures/otio-samples`), 189 (cameras/conditional v2/flatten-extra +
golden demo-3d.otio). Cross-render against melt — test 158
(`apps/kt-cli/tests/cross_render.rs`).

## Old versions are read by the IMPORT, not the reader (R.67, the owner's decision)

- An envelope file `v < KT_META_VERSION` that `timeline_to_project` rejects goes
  through `legacy_import::import_older(text, base)`: JSON preprocessing by the table
  `FIXUPS` (each fix — with the version for which it was created, and with a notice), then the
  ordinary reading. In this case `load_project` returns `Error::NeedsImport { version }`
  — kt-app shows the version and the item `File → Import older Kinetren project…`.
- **The reader does not learn new old forms.** If you find a file that does not read, the fix goes into
  `FIXUPS`, with a test on a synthetic copy of the structure; when support of a version is dropped, its fixes
  are removed together with the tests. The first: `v5_null_morph_targets` (`targets: null` ⇒ "everything
  below"). The existing migrations in the reader (v<3 fractions→ticks, v4 morpher, v8 generator,
  `ease`) remain as they are — this rule is for NEW fixes.

## R.70: exchange doors, passthrough in the project file

- **`import_interchange` — ONE rule for choosing the path** for three inputs: a foreign `.otio` without
  our envelope is read by the ordinary `load_project` (it needs no fixes, and
  `import_older` would reject it — `doc_version` requires `kt.v`); ours of an old version —
  `import_older` on `NeedsImport`; legacy — `import_legacy`. A project file does not go into the door:
  `Error::ProjectFile` names "Open". The application and kt-cli call only it — two copies of
  this `match` would diverge at the first foreign file (and that is how it turned up: t819).
- **`Retained::to_json`/`from_json`** — the passthrough travels in the field `otio` of the project file.
  The `HashMap` fields are written as SORTED PAIRS `[key, value]` (`sorted_pairs`):
  the keys are not strings, the traversal order is random, and without sorting every save is a false diff.
  The order of the FOREIGN keys of the document is NOT touched by this — it is a promise to the export (t154),
  the determinism is given by the insertion-ordered `Map` (`preserve_order`). `is_empty` — "nothing
  to return": such a project carries no field.
- **Media paths: in memory absolute, in the file relative** — this is how it was with `load_project`/
  `save_project`, and the own format repeats the rule via `resolve_project_media`/
  `relativize_project_media` (t797: the project moves together with its material).
- Tests: t814 (the export is self-sufficient, "Open" rejects it), t816/t817 (the round trip of
  passthrough), t146 (detection of one's own file).
