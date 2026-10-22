---
id: F-COL-033
title: How an edit is addressed: id, tick or lineage
status: researched
priority: B
cost: L
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-model, kt-otio, kt-engine, kt-app]
invariants: [3]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-COL-018, F-MED-005]
trackers: []
services: [team-projects, bm-cloud, syncthing]
checked: 2026-09-08
---

# F-COL-033 — How an edit is addressed: id, tick or lineage

## Essence

Two people edited the same edit. To say "this is one and the same edit" or "these are two different ones," one needs to address the edit object itself with something. There are exactly three candidates: an internal number (id), a position in time (tick), and **pedigree** — "a piece of this material, starting from such-and-such a frame, of such-and-such length." The editor sees none of the three and never asks about them, yet this choice decides what happens to their work during a merge, when importing someone else's sequence, and even on a simple opening of someone else's file: an address that did not survive the journey turns comparing documents into guesswork.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none (an event is addressed by its track and time) | there is no shared mode at all, so the question of identity does not arise constructively: an event lives by its position on a track, material by its file path. Copying between projects matches paths `[NOT VERIFIED: Boris FX's primary source gives no description of the internal model]` |
| Adobe Premiere | 26.x | Master Clip + XMP ID | an event on the timeline has no identity of its own: it is a reference to a master clip, and the identity of the master clip is a UUID that the program writes **into the media file itself** (the setting "Write XMP ID to files on import"). Deduplication on importing a sequence runs by the pair "source file + in/out points" |
| DaVinci Resolve | 21 | Media Pool Item; conform by name and timecode | the identity of material is the name plus the timecode of the source; on a collision the program does NOT choose or merge, but puts the letter "c" on the clip and waits for a person. Comparing timelines works one level higher — by sections, not by objects |
| Final Cut Pro | 12.3 | `ref` to an asset inside the library | the identifiers are document-level and are reissued on FCPXML export (`r1`, `r2`, …): they address an object inside ONE document and make sense exactly up to its boundary. Outwards the library is occupied as a whole, so comparing two states is not required |
| After Effects | 26.x | a layer inside a composition | a layer is addressed by its index in the composition, and material by an element of the project; there is no merging, and therefore no requirement that the address survive the journey |

**Outside the core.** Two rows without which the mechanism cannot be described: the format by which the industry exchanges edits, and the only system where divergence is a routine event with a formal outcome.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | (there are no object identifiers) | neither `Clip`, nor `Track`, nor `Marker` carries an identity field: the file holds a name, a range and a reference to material. Any comparison of two foreign `.otio` files MUST rely on pedigree — there is no other carrier in the document |
| Syncthing | 2.x | Folder + Name + Version | a file's identity is the pair "folder and name," and its history is a version vector per device; "the same thing" and "the same thing, but newer" are kept apart and are NEVER derived from the content |

## The trick behind it

**1. Adobe moved the identity of a clip out of the document AND INTO THE MATERIAL.** The identifier of a master clip is not a number inside the project but a UUID that the program on import writes into the media file as XMP metadata; hence the setting "Write XMP ID to files on import," and hence the classic defect: two projects referencing one file produce two unrelated master clips, and support's answer to this question names the cause directly — "these clips have different XMP IDs." The meaning of the construction is visible only against ours: a document does not survive import, while a file does, so the identity moved to where it lives longer. The price — the editor WRITES into the client's material.

**2. Deduplication when pulling in someone else's sequence runs by pedigree, not by name and not by number, and the key includes the edit points.** Premiere does not create a second record if the same file is already in the project — but it does create one if the master clip in two projects has DIFFERENT in/out points. That is, the industry's identity key is the triple "file, start, length" — exactly what we call `media` + `source_in` + `duration`; and its false positives go not toward gluing someone else's clip in, but toward multiplying one's own.

**3. Resolve, on a collision of identities, does not choose a winner but CHANGES THE STATE OF THE CLIP.** Two different materials with one name and one timecode give the letter "c" on the conform page right on the clip; the person chooses the right duplicate with a right-click, and the letter changes to "r." The conflict here is neither a modal dialog nor a file, but a sign of an object that lives in the document until it is resolved — and therefore it is not lost when the project is closed.

**4. The exchange format carries no identity at all.** In OTIO objects have no identifier field; our `ClipId` lives in `metadata["kt"]` and is invisible to a foreign reader, just as Adobe's XMP identifiers are invisible to us. So any comparison between programs relies on pedigree by construction, not by a developer's choice: an id is an address INSIDE one document, and it never crosses the document's boundary.

## Pros

- Pedigree survives what an id does not survive: export, import, conversion, a foreign editor. It is assembled from data that must be in the file anyway, and so it requires no new field.
- Identity in the material (Adobe) makes "the same file" recognisable between projects and between machines without any server — this is the only mechanism in the industry that works on a flash drive.
- A conflict as a STATE of the object (Resolve) survives the closing of the document: the resolution is not tied to the session and is not lost if the person has gone to lunch.
- A number as an address inside a document is cheap and exact: edits that refer to an object by number are applied in one lookup and do not depend on how many objects look alike.

## Cons

- Pedigree does not distinguish two identical pieces: a clip duplicated by copy-paste and its copy are indistinguishable by "file + start + length," and any comparison must additionally tell them apart by their position on the track.
- Identity in the material requires WRITING into foreign files: on a protected card, on a read-only volume and on someone else's material this either does not work or is unacceptable.
- Name plus timecode (Resolve) breaks on `C0001.MP4` from two cards — the most ordinary material; hence "c" on half the clips and manual conform.
- A number reused after deletion is the worst of the three: it does not go missing but LIES, and so its error is not diagnosable by any sign.

## What users say

On how exactly Premiere considers a clip to be the same — the support answer in the thread about multiplied master clips:

> All these imported clips have different XMP IDs, so that Premiere Pro thinks that
> they are not related to each other.
> — Kevin J. Monahan Jr. (Adobe), 04.04.2016,
> https://community.adobe.com/t5/premiere-pro-discussions/solving-duplicate-master-clips-after-they-have-been-edited-into-timelines/td-p/8192178

And the price of mistaken identity, in the words of the same thread's author — it is measured not in inconvenience but in work:

> If we delete one dupe master clip, it deletes itself on whatever timeline it's
> been used with.
> — mjshade, 04.04.2016, the same thread

## How it fits Kinotrain

### What already exists

- **We have pedigree in full and in one place**: `ClipEvent` carries `media`,
  `source_in` and `duration` (`src/libs/kt-model/src/project.rs:1846-1855`), and the identity of the material itself is already computed from its content — xxh3-128 in the `media` table of the index database (`src/libs/kt-media/src/index.rs`). No new field for "file + start + length" needs to be introduced.
- **The number is issued as "maximum + 1" and is reused after deletion**: `src/apps/kt-app/src/app.rs:3111` for clips, `:3160` for transitions, `:3175` for media, `src/libs/kt-model/src/ops.rs:706` for transitions within an operation. A uniqueness guard IN THE DOCUMENT exists and works — `CommandError::IdInUse`
  (`src/libs/kt-model/src/model.rs:296`, checks on `AddClip`, `Split`, `AddTransition`) — but it checks exactly that the number is free RIGHT NOW.
- **Across the file boundary the number passes only in our own document**: `clip_from_otio` takes the `ClipId` from our envelope if the file is ours, and otherwise hands out sequential ones (`src/libs/kt-otio/src/convert.rs:1181-1187`). As in OTIO in general, a foreign file's object has no identity — it is in `metadata["kt"]`.
- **A marker is addressed by TICK, not by number**: two markers on one frame are forbidden (`CommandError::MarkerExists`, `src/libs/kt-model/src/model.rs:318`), and extra ones when reading a foreign file are silently merged by normalisation. The third candidate for addresses is thus already applied by us — and has already shown its price (F-COL-021).
- **Invariant 3 rests on `ClipId`**: `FxStateStore`
  (`src/libs/kt-engine/src/fxstore.rs`) keeps processors by clip number and reuses them while the chain's fingerprint matches.

**The address of the defect found here.** `Retained.clips` — the map of passthrough remainders of a foreign file — is keyed by the same `ClipId` (`src/libs/kt-otio/src/convert.rs:82`), lives outside the model and outside undo (`src/apps/kt-app/src/app.rs:253`, set only on load), and is NOT cleared when a clip is deleted, while the entry is taken by number on every save (`src/libs/kt-otio/src/convert.rs:547`). So on a file that came from outside: delete the clip with the highest number → add a new one → it gets the same number → on saving it inherits the name, the foreign markers, the effects and the inactive media-references of the deceased. The error is silent and is visible only in a diff of the file.

OFFLINE: DEGENERATES — `src/libs/kt-otio/src/convert.rs:1181` — of the three addresses on one machine two work honestly: the pedigree is assembled entirely from the fields of the event, the tick serves as the address of a marker, and the number degenerates into the load order — it survives the round trip only in OUR file; for a foreign one it is issued afresh, and after a clip is deleted it is issued again together with the foreign property from `Retained`, which lies outside the model and outside undo.

NETWORK COST: to say that two edits touch ONE object — edit log — pedigree answers "the same piece of the same material," but does not answer "the same object": two clips identical by copy-paste are indistinguishable, and without a record of the edit itself there is nothing with which to tell them apart.

NETWORK COST: to accept someone else's edit without killing the tail of a reverb — invariant 3 — FX state is keyed by `ClipId`, and numbers are local and reused: a clip with the same number and the same chain structure silently inherits a foreign processor together with its buffers.

### What is missing

- Not a global identifier — we do not introduce one — but **a single rule for issuing numbers**: today "maximum + 1" is written five times in three files, and each place reuses a freed value independently of the others.
- A cleanup of `Retained` on deleting a clip (or a key that cannot be reused by accident): until it exists, the number carries not only the address but also the inheritance.
- A function "this is the same material" that counts by content rather than by path: the hash already lies in the database, but the key of the row remains the absolute path (F-COL-037).

### What gets in the way

The number for us is not an implementation detail but a load-bearing structure: invariant 3 (FX state by `ClipId`), the passthrough of a foreign file (`Retained` by `ClipId`), the links of A/V pairs (`link_group`) and the references of morphers to their targets rest on it. Replacing it with something global means touching all four, and the cost lies not in the schema but in the fact that invariant 3 would have to be rewritten on a different identity and proven again with the FX fix tests. The cheap half is different and lies beside it: to stop REUSING the number and to stop issuing it in five places.

### Which invariants it touches

Invariant 3 — directly and uniquely: it declares a clip "the same" by `ClipId`, and this is true exactly until the number is issued again. The remaining eleven are not touched: addressing computes and draws nothing. It does not touch the rules of the UI checklist at all.

## Estimate

| What | Answer |
|---|---|
| Size | L — the change of the identity carrier itself; the local half (one number generator without reuse + the cleanup of `Retained`) — S |
| Where the main work is | kt-model (issuing and guarantees of the number), kt-otio (what survives the file boundary), kt-engine (identity for FX state) |
| Project format | does not touch it: `ClipId` already travels in `metadata["kt"]`; a monotonic document counter is added there without a version bump (passthrough, F-RES-003) |
| Tests to write | "the number of a deleted clip is not issued again"; "a new clip with a freed number does not inherit the deceased's passthrough"; a comparison of two projects by pedigree on a fixture |
| Depends on | F-COL-018 (what "my edit" means at all), F-MED-005 (relinking — there lies the question "is this the same material") |
| Breaks compatibility | no |

## Verdict

We take the conclusion, not the mechanism. The conclusion of the reconnaissance: **documents can be compared only by pedigree, and we already have it in full** — `media`, `source_in`, `duration` plus the content hash in the index database; no new field needs to be introduced, and a global identifier is needed even less, because the exchange format does not carry it across anyway — neither ours, nor Adobe's, nor OTIO's in general. The only mechanism of the industry in which identity survives the document costs an entry into the client's material, and we do not want that.

One thing must be fixed, and for a reason unrelated to shared work: **our clip number is reused, and together with it the property is reused**. `Retained` is keyed by `ClipId`, is not cleared on deletion, and is substituted on saving by number — so on a foreign file a new clip carries the name and markers of the deceased into the file silently. This is a defect of the single-person session, found by the reconnaissance of shared work, and its cost is one cleanup plus one number generator instead of five.

How we differ from neighbours. **F-COL-018** (Ctrl+Z under shared ownership) is about what undo does when an edit is not only mine; here it is about what an edit is addressed by at all, and this is its premise. **F-COL-023** (marks on a clip) has already noticed that foreign marks of a clip are bound to `ClipId`; there the subject is the carrier of the remark, here — the property of the number itself. **F-COL-021** (two notes on one frame) analyses the price of the second candidate, the tick, and that price has already been paid. **F-COL-037** (pulling in someone else's edit) is about the identity of the MATERIAL at import; here it is about the identity of the EVENT. **F-MED-005** (relinking) is about how to repair the path to material; the question "is this the same file" is common to it and to us, and the answer (hash instead of path) is also common.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/solving-duplicate-master-clips-after-they-have-been-edited-into-timelines/td-p/8192178
  — the XMP ID as the identity of a master clip and the price of its loss (04.04.2016; accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/how-to-copy-sequences-from-another-project-and-not-end-up-with-duplicate-entries-for-source-clips/td-p/5648901
  — deduplication on importing a sequence runs by "source file + in/out points" (15.11.2013; accessed 2026-09-08)
- https://creativecow.net/forums/thread/migrating-to-resolve-dealing-with-duplicated-filen/
  — "c" and "r" on a clip as the state of a conflict of identity in conform (17–18.11.2011; accessed 2026-09-08)
- https://docs.syncthing.net/specs/bep-v1.html — "Folder, Name and Version uniquely
  identifies the contents of a file at a given point in time" (accessed 2026-09-08)
- Our code was checked by grep over `src/` on 08.09.2026: `kt-model/src/project.rs`
  (`ClipEvent`), `kt-model/src/model.rs` (`IdInUse`, `MarkerExists`),
  `kt-app/src/app.rs` (issuing numbers as "maximum + 1," storage of `Retained`),
  `kt-otio/src/convert.rs` (`clip_from_otio`, `clip_to_otio`, `Retained`),
  `kt-media/src/index.rs` (xxh3 of the material).
- The absence of an identity field in OTIO objects was verified against our own implementation of the schemas (`src/libs/kt-otio/src/schema.rs`, the composition of `Clip.2`) and against the set of fixtures `src/fixtures/*.otio`.
