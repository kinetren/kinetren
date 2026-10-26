# Project format — why it is this way

## What hurt and for whom

The old proprietary `.ktproj` was a closed JSON with no compatibility with anyone, and its reader forgave errors: a broken field meant "an empty project", and a user who saved such an "empty" lost the edit. The owner demanded a full move to OpenTimelineIO: "read and write other people's files, do not break what we cannot read there when saving, and add our own so that nobody breaks ours either". No living Rust implementations of OTIO were found (stubs, FFI to C++, AGPL) — the format was implemented ourselves, with techniques borrowed from the reference C++/Python library 0.18.x: a typed metadata envelope, passthrough of unknown schemas, `OTIO_SCHEMA` versions.

Then it hurt as it grew: a real film (Netflix "Sol Levante", 23.976, an hour-long timecode) was imported as 30 fps and drifted by an hour into a 162-second file; the owner's file with 1952 clips and 76 overlaps could not be written into one track (OTIO does not express overlap) and could not be expanded into 40 tracks; one wrong byte in a morpher, typed in by hand for a live scenario, opened the project with no tracks and no settings; halves of one cut drifted into someone else's editor in different coordinate systems; a recovery file with three nested reels grew to 115 MB.

## Decisions and why

- **The namespace `kt`, not `kinotrain`** — a direct decision of the owner; the envelope version is one per file (`Timeline.metadata.kt.v`), nested envelopes carry no versions.
- **Idempotence instead of byte stability.** Byte-passthrough of someone else's file is not promised: the first save normalizes the formatting exactly as the reference does. What is promised is `save(load(save(x))) == save(x)` — this is enough to distinguish data substitution from reformatting, and it is checked in every round by all the crate's tests.
- **"Do not understand halfway".** Either the schema is ≤ the supported version and is typed entirely, or it is a whole raw block. The spine (Timeline/Stack/Track) being newer — a refusal, because the positions of a track's children are the order, and it cannot be interpreted partially.
- **Version by content.** "Backward compatibility is not needed" — the owner's decision, repeated in every round; but reverse reading (files before vN) is kept, and forward — an honest refusal `KtMetaTooNew`. The writer sets the OLDER version at which an old build would not spoil the file silently: each step v2…v9 is established exactly for a novelty for which an old build would have had neither a refusal nor a noticeable stub. One Pan3d does not raise the version: an unknown effect routinely degrades to foreign passthrough. The generator in R.52 also did not raise the version (an old build saw a noticeable stub), but in R.59 it began to raise it — the values moved to the event, and there would be no stub anymore, there would be corruption.
- **Broken — refusal, someone else's — passthrough.** "The section is absent" and "the section is there and broken" are different outcomes. A notice does not save here: a silently reassigned clip number breaks link groups and morpher references irreversibly.
- **Transitions in the model, not conversion at the format boundary** (the owner's decision): clips butt against each other, explicit `Transition.1`; outward a meaningful `transition_type`, a foreign one is not substituted. An edge transition and a transition next to a Gap — a fade (the reference's semantics: a Gap is substituted for a missing neighbour), rather than "drop with a notice".
- **Layers — as neighbouring `Track`s.** To the owner's direct question "is it really impossible to write with overlaps, ignoring the standard?" — really impossible: a clip in OTIO has no field "start on the timeline". The `kt.layers` map gathers the tracks back; if it does not converge — a notice and the tracks one to one.
- **A nested reel — a `Stack` as a child of a `Track` with the body in each instance**, as Premiere and FCP do: another editor sees the edit, not a reference into our envelope. The sidecar `kt.sequences` is advisory (the pattern of the same layer map): if it is lost, the edit is read from the file.
- **Paths are relative, fallback `file://`** — the owner's decision "fallback/absolute only if it is in other editors" (Resolve/Hiero write `file://`).
- **Legacy `.ktproj` — import only** through a separate item; the `.ktproj` writer is removed, so that the format has no second writer. The golden for import is frozen byte for byte.
- **Atomic write lives in kt-otio** — one home for GUI and CLI; the order "file → rename → directory" is checked by injection, not by fault injection in the core.

## Rejected and why

- **A ready-made OTIO library for Rust** — rejected: none is alive (stubs from 2020, FFI to C++, the only pure Rust one is under AGPL).
- **Ignoring the standard and writing overlaps into one track** — rejected: overlap in OTIO is physically inexpressible; writing overlaps one after another stretched the owner's edit from 6332 to 6410 s.
- **Importing legacy with a split of 3+ overlaps across tracks with volume compensation 1/N** — rejected by the owner: a split without compensation, with a report; and with layering (the default) the import does not touch anything at all — his project was unfolding into ~40 tracks.
- **Discarding an unreadable envelope with a notice** — rejected: the consequences (a new clip number, broken groups) are irreversible, and a notice does not fix them.
- **Storing `available_range` only in the clip's envelope** — rejected: an event born of an edit has no envelope; references to material became a property of the media.
- **Distributing markers on a cut in the model** — rejected: undo would return the parent without markers that had moved to the children; the writer distributes them by the material window.
- **Moving the clip's envelope into the model** — rejected: kt-model keeps the rule "not raw JSON", and the event is cloned on hot paths.
- **Refusing to write on a failed `fsync` of the directory** — rejected: on exFAT and network shares this is legitimate, the file is in place; lying to the user that nothing was saved is worse.
- **Merging two diverged files (F-COL-029 "nobody merges")** — rejected: an event is addressed by the pair "track × time", both coordinates are changed by someone else's edit that did not touch the event; `ClipId` in the file is an address inside the document, not the name of a thing, and it is issued as "maximum + 1" with reuse; the file has no document revision — there is only the format version. Merging two edits is for us not a contested state but a document that validation rejects. None of the five commercial editors has a merge — the industry bought prevention (locks).
- **Chat next to the document (F-COL-028 "chat next to the document")** — rejected: chat is the first surface obliged to live at rest (repaint only on movement, invariant 12; a permanent strip for rare text is forbidden by the UI rules). This is not a cost and not an absence of network. The lesson of the form, still valid for the format: everything that comes from a second person enters as DATA, which never writes into the timeline by itself.

## Non-obvious details

- **The shape of a segment is written to the file BY CONTENT** (R.61, its own `Serialize` for `ParamCurve`, `src/libs/kt-model/src/project.rs`). A curve whose keys all carry one expressible shape is written in the former `ease` field and does not move the version; the rest travels in a `shape` field on the key and raises the file to v10. The reason is not backward compatibility, which the owner refused, but that `ease` was ALWAYS written: simply deleting the field would shift the bytes of every file with curves and would break the version cascade across all fixtures and the golden `demo-3d.otio`.

- Refusal of the spine is done by a PREFLIGHT on the raw `Value` before typing (`src/libs/kt-otio/src/schema.rs`, `preflight_backbone`): the serde bridge of polymorphic children turns our typed error into an addressless `Json(custom)`, so inside `typed()` the refusal is no longer distinguishable. The recursion of the preflight does not enter unknown schemas — their content is passthrough and is not interpreted.
- The canonical key order requires `serde_json/preserve_order`, and this feature is GLOBAL for the workspace (cargo unification): `serde_json::Map` everywhere becomes an IndexMap. New consumers of `Value` have no right to rely on alphabetical order (`src/libs/kt-otio/src/io.rs`, `write_document`).
- `OTIO_SCHEMA` must be on EVERY serializable object, including the nested `RationalTime`/`TimeRange` (manual Serialize with a tag, `duration` before `start_time`) and the `Stack` in `Timeline.tracks` (the field goes past the Composable bridge via `serialize_with`). Our idempotence tests do NOT catch this — our own "incomplete" is forgiven; only conformance with the reference catches it (`src/libs/kt-otio/tests/otio_conformance.rs`, `src/libs/kt-otio/src/schema.rs`).
- Our ticks are written with `value` as a whole number as f64 (exact up to 2^53), `rate: 90000.0`; foreign rates are quantized by `round(value*90000/rate)`. NTSC remains a drift of 0.1 % on the frame grid, but splices in ticks do not suffer; byte stability of the round trip on NTSC files does not hold (a tick per clip) — `grid_fps` in `src/libs/kt-otio/src/convert.rs`.
- The model's `source_in` is from the FIRST frame of the file, the OTIO `source_range` is from the start of `available_range`; subtraction on read / addition on write, and the media fallback for newborn ones — `src/libs/kt-otio/src/convert.rs` (`Retained.media`). A clip without `source_range` takes the material whole with a ZERO entry point — here the start of `available_range` is a timecode, not a frame skip.
- A broken section is distinguished via `kt_get_checked` (`src/libs/kt-otio/src/meta.rs`): `None` — the section is absent, `Err` — it is present and broken. A return to `kt_get(...).ok()` in any of the three places (Timeline, clip, attachment) — a red t251/t383.
- The version cascade — one `if/else` in `src/libs/kt-otio/src/convert.rs`
  (`project_to_timeline`), from the highest to the lowest; the v9 predicate is shared for the generator on the event, the public set and the overrides. A new condition placed in the wrong spot raises the version for all files in a row — only t433b catches it, walking ALL committed fixtures (`src/fixtures/*.otio`); t148/t189 guard one reference each.
- Unknown fields of OUR envelope pass through flatten-extra (`KtTimelineMeta`,
  `KtClipMeta` in `src/libs/kt-otio/src/meta.rs`): an addition of the next version does not need a hard refusal, if the old build does not spoil it silently. The empty `kt` envelope of a transition is removed entirely — otherwise a foreign file after a return to a crossfade stops being byte-stable.
- Default track labels (V1/A1) are NOT written to `Track.name` — only explicit names; otherwise a foreign file after a round trip grows fictitious names. Event level 1.0, natural row order, a layer map without overlaps, `kt.start` when it coincides with the track's cursor — are not written for the same reason (`src/libs/kt-otio/src/convert.rs`).
- Layer 0 of the layer map carries the track name and foreign extras/gaps — otherwise the stratification would duplicate them. Foreign raw data of TRACKS is returned to its place only at the root: the key `(kind, lane)` inside a reel would collide with the root one (`src/libs/kt-otio/src/convert.rs`, `read_reel`).
- On reading, the track number becomes the RANK of its row, and `Retained.tracks` is reordered by the same permutation — otherwise a track's foreign metadata would silently drift onto the neighbouring track (`src/libs/kt-otio/src/convert.rs`, t593).
- Both halves of the converter are recursive (`reel_children` / `read_reel` in
  `src/libs/kt-otio/src/convert.rs`): the generator, layers, transitions and morphers inside a reel go through the SAME code as at the root. `bin_media` counts "used" over the WHOLE tree — a file standing only inside a reel would otherwise get into both the bin and the reel, give a duplicate address and a load refusal.
- A foreign attachment without tracks (`Stack` of clips, `Track` inside `Track`) is wrapped in a synthetic `Track` of the parent's kind and goes into the same `read_reel`; an attachment with sound and picture becomes a pair of events in one link group — on the timeline there is one event more than in the foreign file, and this is stated in a notice (`src/libs/kt-otio/src/convert.rs`).
- A foreign `generator_kind` is guarded directly in `decode_media_ref`
  (`src/libs/kt-otio/src/schema.rs`): typing it would mean losing the byte-idempotence of foreign files. Parsing of the pseudo-paths `gen:`/`synth:` belongs only to the `kt_media::factory`; kt-otio carries them verbatim (`src/libs/kt-otio/src/url.rs`, `media_target_url`).
- Migrations of old files are one-way and live in the reading (`src/libs/kt-otio/src/convert.rs`, `timeline_to_project`): fractions → ticks by the geometry of the clip (in a reversed one the fraction is mirrored), morpher before v4 → `timed: false`, `ui.lane_order` → model, generator values from `parameters` → event curves `kt_model::text_anim_curves` — the same ones the preset in the window sets (t665 checks against it).
- The temporary write file is the full name plus `.<pid>.tmp` (`src/libs/kt-otio/src/io.rs`, `atomic_tmp_path`): `with_extension` ate multi-dot names, and without the pid two copies of the program would write into one tmp, and a `rename` of one would replace the project with half of another.
- The round trip "loaded → EDITED → saved → loaded" (`src/libs/kt-otio/tests/edit_roundtrip.rs`) — the only one in the crate where an edit happens at all; the other round trips check an untouched project and do not see losses of newborn events.
- Limitations, documented and not implementable: a `SerializableCollection` root — refused; `.otioz`/`.otiod` — no; binding of foreign leftovers of tracks/transitions by position at load time may re-glue foreign metadata after structural edits (clips are not affected — the ClipId key) — `src/libs/kt-otio/CLAUDE.md`, section "Limitations v1".

## R.67: old versions are read by the import, not by the reader

- **The owner's decision, verbatim**: "make an item in the File menu for importing old versions, and fix it precisely in the import so that the reader can read it normally. And fix it for other agents too so that they automatically make the corrections into the import of old versions by themselves; these corrections will gradually be removed as old versions retire". Hence `legacy_import.rs`: a table of fixes, each with the version for which it was made; dropping support for a version removes its fixes together with their tests. The reader does not learn the new forms of old files — otherwise "backward compatibility", which is "not needed", would accumulate in it forever.
- **The refusal names the door.** A file of a version below the current one that the reader does not read is rejected with `NeedsImport { version }`, and the text carries the name of the menu item (rule 19). Previously the person saw "metadata.kt is unreadable: data did not match any variant of untagged enum Repr".
- **The first two fixes** — from the owner's project `untitled2_v26.09.26`: `targets: null` in a morpher (v5) and the retired effect `kt:pan` inside the morpher's PATCH (v6), where it lay as a bare `FxInstance` and the untagged reader of the patch failed entirely.

## Deferred

- **Path mapping after the Resolve pattern (F-PRJ-003 project-portability, stage 3)** — deferred until a second participant: today it is one user on one machine, and a relative path covers the scenario "everything moved together". A caveat: as soon as a user storage outside the project appears, a switch "put inside the project" is needed on the same day.
- ~~Cross-platform paths in `target_url`~~ — done in windows-build (2026-10-26), see the section below.
- Moving foreign attachments into the strict tier of the cross-render (t158) — after the placeholder stops differing from the composite of the foreign editor.

## R.70: the project leaves the interchange format

**What hurt.** The JSON of OpenTimelineIO has no object identity: there is nothing to say "this is the same reel as above"; only nested content is expressed, and the body of a reel was written anew into each of its instances. For the owner, three reels gave 15 220 clips and 115 MB, which the autosave wrote every half minute. Measurement against the serialization of the model: 17× on a flat project, 501× and 929× on nested scenes (02.10.2026). The owner's decision, verbatim: "we are leaving the otio format, we keep otio only for export and make breaking changes so that our format becomes a hundred times better", and right after: "I am against binary junk — our file must be indexable in git".

**Decisions.**

- *The project file — serialization of the MODEL* (`kt_model::project_file`): "every entity once" is a property of the model itself (`Project.sequences` holds the body, the instance refers by address); the format simply does not spoil it. The layer "own schema on top of `Value`" was rejected — this is exactly what `metadata.kt` was.
- *Line-by-line JSON, not compact.* Compact was in the first variant of the plan and would have given 501×, but one line for the whole file is not a file for git: editing one event "changes" everything, and `diff`/`blame`/merging are dead. Line-by-line costs 2× (234× instead of 501× on the same scene) and makes the owner's requirement MEASURABLE: editing one event changes units of lines, and one model gives the same bytes (t822). Binary and compression are forbidden outright; Premiere's compression on save was exactly what became a performance problem.
- *Determinism without substituting the order.* The only source of randomness is the `HashMap` fields of foreign passthrough; they are written as sorted pairs. The first revision of the plan promised "passthrough by sorted keys" across the board — this would have broken t154: the order of FOREIGN keys is a promise to the interchange file. Determinism of the insertion-ordered `Map` is given by itself (`serde_json/preserve_order`, the feature is unified across the workspace).
- *The version — a number, refusal forward, migration backward.* A partial read is forbidden ("it opened, and part of the edit is missing" — the worst of three outcomes, F-RES-003). Breaking changes are allowed by the owner; there is no compatibility cascade with a foreign schema here.
- *The extension `.kt`, a kind by content.* `.ktproj` is not reused: two formats under one suffix is a trap, and the legacy `.ktproj` lives on in the import.
- *Inline was not cancelled, but got an addressee.* "The body of a reel in each instance" is right for EXPORT — a foreign reader does not know our conventions. The earlier decision was not a mistake: it was correct for an interchange file and was applied to a file that only now has stopped being an interchange file.
- *Doors.* "Open" accepts only `.kt`; an interchange file and a legacy one are rejected with the file's kind and the name of the import item (rule 19). The import is ONE door for three entrances (`kt_otio::import_interchange`): a foreign file without an envelope is read by the ordinary reader, ours that is old — by fixes, the legacy one — by its own reader; the result is an UNNAMED document with the hint "`<name>.kt` next to it". Writing `.kt` under the name `.otio` or silently putting it next to it is not allowed — the person chooses the name. Export is a separate item `File → Export → OpenTimelineIO…`; it touches neither the project file nor the asterisk.
- *Media paths: absolute in memory, relative in the file* — the rule of `load_project`/`save_project` repeated for its own format (t797): the project moves together with its material.

**Rejected.** The R.68 variant "expand the attachment by one level" — relied on the premise that the bodies of reels already lie in `kt.sequences`; there is no field with clips there, and truncation would have taken the content away from OUR reader. Deduplication inside OTIO with our own envelope — both chairs anyway. A database instead of a file (the Resolve path) — another round. A silent opening of `.otio` "as before" — "it opened, but what you are working in is unclear".

**What we pay.** A foreign editor will not open `.kt` directly — a trade that everyone made (Resolve, FCP, Premiere); the export door is named and checked live. The durability of the format is now ours: the version, the field-by-field and byte-by-byte round trip, atomicity and the revision guard are not touched. The OTIO golden fixtures remain and freeze the EXPORT.

## Renamed to Kinetren: the marker `kinetren`, the former `kinotrain` via the import (2026-10-21)

The owner chose a full rename, including the root marker. Per the owner's rule (R.67),
the reader is NOT taught the former key: `FileKind::FormerName` makes Open refuse the
file and name the door, and the import (`from_former_text`) renames the key and reads the
rest unchanged, so the result equals the same project written today (t828/t829/t832).
Rejected: accepting both keys in the reader. It is friendlier for one release, but it
teaches the reader an old form forever, and dropping the alias later would break files
silently.

## Windows and cross-OS paths (windows-build, 2026-10-26)

The deferred item "cross-platform paths in `target_url`" is done (`src/libs/kt-otio/src/url.rs`):
string rules with an explicit `PathFlavor`, so the Windows rules are tested on Linux and the
other way round (t854–t856). Valid `file:///C:/…` and UNC `file://host/share/…` URLs; only the
drive (or share) in common falls back to an absolute URL, as only the root does on Linux; case
and the verbatim prefix are ignored on Windows. "Absolute" on reading is the union of both OSes:
`/home/u/a.mp4` read on Windows (or `D:\x.mp4` on Linux) is never joined to the project folder —
it becomes a placeholder showing that path and is written back unchanged. Resolve-style path
mapping (F-PRJ-003) stays deferred: this only stops paths from turning into wrong ones.
