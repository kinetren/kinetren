---
id: F-COL-037
title: Pulling someone else's edit into your project
status: researched
priority: B
cost: L
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: no
crates: [kt-otio, kt-media, kt-model, kt-app]
invariants: [8]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-COL-033, F-MED-005]
trackers: []
services: [premiere-productions, bm-cloud, fcp-shared-library]
checked: 2026-09-08
---

# F-COL-037 — Pulling someone else's edit into your project

## Essence

An editor received a project file from a second editor and wants to take a scene from it — not all the work, but these four minutes. They do not expect a merge and do not want one: they want the other person's sequence to lie NEXT TO their own, whole and as it was, and the material not to multiply. This is the only form of shared work that everyone uses every day, and it merges nothing — it lays things side by side. All the complexity moves to one question: is the other person's `C0001.MP4` my `C0001.MP4` or a different one?

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | nested project (`.veg` as an event) | the other person's edit is placed on a track as ONE event and remains a separate document: the question of deduplicating material is not resolved but postponed `[NOT VERIFIED: there is no description of the internal model from Boris FX]` |
| Adobe Premiere | 26.x | Import Project / importing a sequence from the Media Browser | the sequence arrives together with ALL the material it refers to; deduplication is governed by two settings — "Allow duplicate media during project import" and "Write XMP ID to files on import" — and the key is the pair "source file + in/out points" |
| DaVinci Resolve | 21 | Import Timeline (AAF/XML/EDL/DRT), conform | the timeline is pulled in separately from the material: first the media pool, then binding by name and timecode. A name collision gives a "c" on the clip and manual resolution |
| Final Cut Pro | 12.3 | copying an event between libraries | the unit of transfer is the whole event, together with its material; the choice "copy files or reference them" is set during the transfer; deduplication by path `[NOT VERIFIED]` |
| After Effects | 26.x | File → Import → a `.aep` project | a foreign project arrives as a FOLDER inside the current one, with its own material items; the items are not reconciled `[NOT VERIFIED]` |

**Outside the core.** One row: the format that makes such an import possible between different programs, and which also explains why deduplication always falls to the receiving side.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | `ExternalReference.target_url` | the material in the file is an ADDRESS, not content and not an identifier: the reference has a URL and an available range, and nothing by which two addresses could be recognised as one file |

## The trick behind it

**1. What everyone calls "merging projects" is an import, and it merges nothing.** The other person's sequence arrives whole and lies beside one's own; questions of "whose edit is newer" and "what has changed" do not arise at all, because both versions remain on screen at once. A practical conclusion follows that turns the topic around: the most demanded form of shared work turned out to be the one that requires neither a server, nor a revision, nor a log — it requires exactly one thing, the identity of the material.

**2. The settings by which Adobe controls the import of a sequence are not about editing but about writing into foreign files.** For the import to go through cleanly, deduplication must rely on the XMP identifier, and that appears in the material only if the person, when importing the material itself, has permitted the editor IN ADVANCE to write into files. That is, the cleanliness of today's import is determined by a decision taken a month earlier and for another reason; hence also the advice "turn on both settings," which has been repeated on forums for years.

**3. The deduplication key is pedigree, and it errs TOWARD MULTIPLICATION.** Premiere will not create a second record for a file already in the project, but it will create one if the same file has different in/out points in two projects. A false positive directed toward duplicates was chosen deliberately: an extra record is irritating, while two different takes glued together by mistake spoil the edit silently.

**4. And what all of this breaks on is the file name.** Cameras number from zero on every card, so `C0001.MP4` can occur ten times in a project; for Resolve, where identity is name plus timecode, this gives a "c" on half the clips and manual conform. There is no ready answer anywhere: the industry practice is to rename the material BEFORE editing, that is, to repair the identity by hand while it is still cheap.

## Pros

- "Laying it side by side" requires neither a revision, nor a log, nor a second participant at the moment of transfer: the file arrived, the scene is in place, both versions are visible.
- The unit of transfer is the sequence, that is, exactly what people share work by; the question "who owns what" resolves itself.
- Deduplication by pedigree that errs toward duplicates is safe: it cannot lose material, and an extra record is visible to the eye.
- The import is reversible: if you do not like it, delete the sequence; your own edit is untouched.

## Cons

- Material multiplies: two records for one file means two effect chains, two sets of interpretation settings and two places where the path must be repaired.
- Identity by name breaks on the most ordinary material from cards, and the cost of an error is asymmetric: separating them is an irritation, gluing them is a spoiled edit.
- Identity by content requires reading the whole file, and material can sit on a slow volume and run to terabytes.
- The import pulls in EVERYTHING the sequence refers to: four minutes of edit bring with them all the material they were assembled from.

## What users say

The formulation of the task, verbatim and twelve years before our reconnaissance:

> I don't want to keep two entries of the same source clips in the project (in
> different bins too), because that gets confusing down the road.
> — Zooropa75, 15.11.2013,
> https://community.adobe.com/t5/premiere-pro-discussions/how-to-copy-sequences-from-another-project-and-not-end-up-with-duplicate-entries-for-source-clips/td-p/5648901

And what the task splits into on real material:

> There are several clips with duplicated filenames.
> — Rodrigo Silvestri, 17.11.2011,
> https://creativecow.net/forums/thread/migrating-to-resolve-dealing-with-duplicated-filen/

## How it fits Kinotrain

### What already exists

- **An import of someone else's edit ALONGSIDE one's own does not exist at all.** The only import item in the program is `.ktproj`, and it does not pull in, it converts: it reads the old file, writes a new `.otio` and opens it INSTEAD of the current project
  (`src/apps/kt-app/src/app.rs:5411-5460`, `*self = app` at the end). In `enum Command`
  (45 variants) there is not one that would introduce a foreign edit into the document.
- **We do have material deduplication, and its key is the address.** When reading a file, clips share one bin record if `target_url` matches
  (`src/libs/kt-otio/src/convert.rs:1171`); clips without a reference deliberately do NOT share a record, so that an empty URL does not glue independent items together. The URL is relative (`src/libs/kt-otio/src/url.rs`), so the key is "the path relative to the project folder": the same file arriving by another path gives a second record, while different files lying identically relative to their folders give one.
- **The identity by CONTENT is already computed and already lies beside the project**: xxh3-128 in the `media` table of the index database, plus size and mtime as a fast path (`src/libs/kt-media/src/index.rs:151-190`). The hash is computed once on insertion.
- **But it is subordinated to the path**: the row is looked up by the ABSOLUTE path of the file, and the hash is compared only inside the row already found — in order to rescue a record whose mtime has changed. Two files with identical content at different paths never meet in this table.
- **The companion, moreover, is disposable by design**: on any opening error `.ktindex.db` is deleted and created anew, and the row key is the absolute path, so moving the folder resets the index entirely (F-COL-002).

OFFLINE: DEGENERATES — `src/libs/kt-media/src/index.rs:168` — the pulling-in degenerates into "open someone else's file instead of your own," while the hard half — recognising material — is already written and deliberately subordinated to the path: content is compared ONLY inside a row found by absolute path, and it serves to rescue a record whose mtime has changed. The key by which the industry would like to deduplicate is computed by us and is not used as a key.

NETWORK COST: to learn that someone else's `C0001.MP4` is not mine — media address — today two records are counted as one material by matching relative path, that is, by the name of a folder on someone else's machine, and by content no one compares them.

NETWORK COST: to bring someone else's scene without bringing its bytes — cross-project storage — the import drags in all the material the sequence refers to, and we have neither a common place for it nor a form of "took the edit, the material stayed there."

### What is missing

- A command "pull someone else's edit alongside": today even on one machine there is no way to take a scene from yesterday's copy of the project, except to open it and copy the selection by hand through a second window.
- A second key of the `media` table — by content: the hash is computed, there is no index on it, and "the same file in another folder" is not recognised.
- Rules for resolving collisions on insertion: for NAMES we already have one (a suffix on pasting regions, item 263), and for material there is none.

### What gets in the way

Three things, and all of them constructive. The first: numbers. An imported edit brings its own `ClipId`, `MediaId`, `GroupId` and morpher references to targets, and all of them are local and reusable for us (F-COL-033) — so the insertion must renumber an entire connected subgraph, not a list of clips. The second: `Retained` — the passthrough of a foreign document is keyed by the same numbers and lives outside the model, so "pulling in" also means merging two maps of remainders. The third: deduplication by content requires READING the file in full, and material can sit on a slow volume; this cannot be done on the UI thread, and so recognising material is asynchronous work with an intermediate state "don't know yet."

### Which invariants it touches

Invariant 8 is the only one and it is to the point: it holds the rule "the interface does not generate decoding," and deduplication by content is a cross-cutting read of the file; it belongs in the same index worker where hashing already lives, and in no way in the insertion gesture. The remaining eleven are not touched: the import computes nothing in the frame and does not touch sound.

## Estimate

| What | Answer |
|---|---|
| Size | L — insertion of a connected subgraph with renumbering plus recognition of material; a separately taken second index key — S |
| Where the main work is | kt-model (insertion of a subgraph, renumbering, name collisions), kt-media (index by content), kt-otio (merging `Retained`), kt-app (a menu item and the choice of what exactly to pull in) |
| Project format | does not touch it: the result of the import is an ordinary project |
| Tests to write | "a pulled-in sequence does not break the numbers of its own and of foreign objects"; "the same file by another path gives ONE bin record"; "`C0001.MP4` from two cards remains two records"; a round trip after the import |
| Depends on | F-COL-033 (identity of the event), F-MED-005 (relinking — the same question "is this the same file"), F-PRJ-001 (several edits in one document) |
| Breaks compatibility | no |

## Verdict

We take it — and take it as a SINGLE function, because that is what it is: importing someone else's sequence merges nothing, does not require a second participant, and is equally needed by a person who takes a scene from their own yesterday's copy. This is the most useful finding in the section "merge": the only form of merging people use is not a merge at all, and therefore it is fully available to us.

A concrete first step follows from the reconnaissance, and it is not about editing. **The key by which the industry would like to deduplicate material is already computed by us — xxh3-128 of the content in the index database — but it is subordinated to the path and therefore does not work as a key.** Making it a second key of the `media` table costs one index and one query, and gives three answers at once: "the same file moved to another folder" is recognised without a server (and removes half the pain of F-COL-002 and F-MED-005); "`C0001.MP4` from two cards" stops being glued together, because the content differs; and importing someone else's sequence gets a key that survives someone else's file system. The limitation is named at once, so that it is not discovered later in a round: a hash requires a cross-cutting read of the file (invariant 8 — only in the index worker), and it does not recognise re-encoded material.

What we do NOT do: we do not try to merge anything at import and do not show a "conflict of sequence versions." Both sequences remain side by side in the document, and the person sorts them out — exactly as all five do.

How we differ from neighbours. **F-PRJ-001** (several edits and a shared bin) is about several sequences living in ONE document; this is a precondition, while the subject here is how a FOREIGN one gets there. **F-PRJ-003** (a project on another machine) is about the portability of one's own document; here the second document is not transferred anywhere, it is pulled in. **F-MED-005** (relinking and consolidation) is about repairing the path to material; the question "is this the same file" that it shares with us is answered here by the same hash, but on a different occasion. **F-COL-002** (service files beside the project) has already shown that the index key is the absolute path; here this same property hinders not the transfer but the recognition. **F-COL-033** is about the identity of an EVENT; here it is about the identity of the MATERIAL.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/how-to-copy-sequences-from-another-project-and-not-end-up-with-duplicate-entries-for-source-clips/td-p/5648901
  — the statement of the task and the deduplication key "source file + in/out points" (15.11.2013; accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/solving-duplicate-master-clips-after-they-have-been-edited-into-timelines/td-p/8192178
  — the XMP ID as a condition of a clean import and the price of its absence (04.04.2016; accessed 2026-09-08)
- https://creativecow.net/forums/thread/migrating-to-resolve-dealing-with-duplicated-filen/
  — duplicate names from cards and manual resolution of a conform conflict ("c" → "r") (17–18.11.2011; accessed 2026-09-08)
- Our code was checked by grep over `src/` on 08.09.2026: `kt-app/src/app.rs`
  (the only import item — `.ktproj`, converted and opened instead of the current one),
  `kt-otio/src/convert.rs` (`media_by_url`, deduplication of bin records),
  `kt-otio/src/url.rs` (relative URL), `kt-media/src/index.rs`
  (`ensure_media`, xxh3-128, the row key — the absolute path),
  `kt-model/src/model.rs` (the composition of `Command` — not a single import command).
