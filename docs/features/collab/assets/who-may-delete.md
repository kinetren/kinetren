---
id: F-COL-006
title: Who has the right to delete media
status: candidate
priority: B
cost: S
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-app, kt-engine]
invariants: [9]
ui_rules: [16, 19]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [premiere-productions, resolve-project-server, fcp-shared-library, bm-cloud]
checked: 2026-09-08
---

# F-COL-006 — Who has the right to delete media

## Essence

In a shared storage deleting is more dangerous than editing: a bad edit is visible and can be
undone, but deleted material is not undone by anything and is discovered by ANOTHER person, on
another day, as a black frame in the middle of a finished edit. Hence the question that each editor
answers in their own way: who has the right to erase a file, what the program uses to prove that it
is no longer needed, and what it does when it is nevertheless gone.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Project Media → Remove from project | removes from the project, does not touch the file; the question "who has the right" is not asked at all — there are no rights, there is a file system — **[NOT VERIFIED]** from the 2026 product line documentation |
| Adobe Premiere | 26.x | Make Offline… → Media Files Are Deleted; Productions (a lock on the project file) | the only one of the five that deletes the BYTES via a checkbox in a dialog, and deletes irreversibly. The reference count at the same time does not even cover its own document as a whole: in a Production, closed projects are unaware of actions in open ones |
| DaVinci Resolve | 21 | Remove Unused Clips (18.6+); Delete File(s) from Disk in the Media Pool | the reference count is done honestly and expensively: "This action loads all timelines, compound clips, and Fusion compositions, then analyzes them for media usage" — and takes minutes. The scope of the count is ONE project; deleting a media pool clip that a timeline refers to, according to forum evidence, passes without a warning — **[NOT VERIFIED]**, the primary source is unavailable |
| Final Cut Pro | 12.3 | Delete Generated Library Files; the library as a whole | the material lives inside the library, and the library is handled AS A WHOLE: "The document [Library name] is already in use by [user] on [Mac].local". There is no right as such — there is an exclusive lock on everything at once |
| After Effects | 26.x | Remove Unused Footage; Collect Files | removes unused items from the project, does not delete files; the knowledge of usage is limited to one document by construction of the model |

## The trick behind it

**1. All reference counters count references inside ONE document, and Resolve is the only one
that says what this costs.** The wording of the help is "loads all timelines, compound clips, and
Fusion compositions, then analyzes them for media usage"; the operation takes minutes and, more
importantly, "does not delete any clips from your disk". That is, the industry answers the question
"who has the right to delete" like this: NOBODY deletes bytes; only REFERENCES are deleted, and even
for that one has to raise the whole document. Everything that lies beyond the boundary of this
document is invisible not because of an oversight but because of the design.

**2. Adobe goes further and admits that the count does not even cover its own document.**
In a Production "closed projects in a Production are not aware of actions you take in other open
projects" — that is, the scope of the count has narrowed from the document to its OPEN PART. The
granularity of the lock (the project file) and the granularity of knowledge about usage are DIFFERENT,
and the second is smaller than the first: the whole project is locked, but the program knows only
about what is currently open.

**3. A consequence follows which is absent from every feature list: the guard "material is in use"
lies in DIFFERENT directions for oneself and for the neighbor.** For oneself it is too strict — it does
not allow removing media that is referenced only by my own document, although I am the one who sees it
best. For the neighbor it is too lax: it knows nothing about their references and allows deletion. One
and the same counter, two opposite errors — and this is not a balance, but a direct consequence of the
fact that the unit of counting is the unit of a RECORD, not the unit of material.

**4. Real deletion of bytes remains a checkbox in a dialog and is not undoable.**
"Media Files Are Deleted" stands next to "Media Files Remain on Disk" in one list, differs by one word,
and the price of a mistake is the source files of a take as a whole: "This action cannot be undone and
Ctrl+Z do nothing".

## Pros

- A reference count per document gives an exact answer where the document is the whole world:
  in solo work "unused" is computed completely and without guesses.
- The refusal to touch the disk ("does not delete any clips from your disk") makes the operation
  reversible: you removed the excess from the project — the files stay in place, and it can be undone
  by import.
- The exclusive lock on the whole library at Apple, for all its crudeness, is honest: while the library
  is held by one person, the question "is the material not needed by a second one" does not arise at all.
- The explicit separation of "remove from the project" and "delete from disk" is the only thing that
  really saves: two commands, not one with consequences.

## Cons

- The cost of an honest count is high: loading all the timelines of a project takes minutes,
  so it is done rarely and "for archiving", not at every deletion.
- The scope of the count does not coincide with the scope of storage. The material is shared, the count
  is per project; no lock on the project fixes this.
- No one of the five has a right as an entity: "who has the right to delete" is decided by the file
  system, that is, by whoever has access to the folder.
- Deletion of bytes is available from the editor and is irreversible — even though the information
  the editor has for the decision is certainly less than what is needed.
- The consequence surfaces not there and not then: for a second person, when opening their own document,
  in the form of offline clips with no indication of who removed them and when.

## What users say

The boundary of knowledge was named by an Adobe employee, answering "source media has been
deleted or moved":

> closed projects in a Production are not aware of actions you take in other
> open projects
> — Ben Insler (Adobe Community Manager), 13.07.2024,
> https://community.adobe.com/t5/premiere-pro-discussions/productions-source-media-has-been-deleted-or-moved/td-p/14718332

The price of one checkbox, verbatim, in an answer marked as correct:

> This action cannot be undone and Ctrl+Z do nothing.
> — Averdahl, Adobe Community, thread «deleted media», 21.09.2024,
> https://community.adobe.com/t5/premiere-pro-discussions/deleted-media/td-p/14873391

The author of that thread (nikitar28785003, 21.09.2024) ticked «Media Files Are
Deleted» without understanding its meaning, and lost the source files of DJI Ronin 4D; further in the
thread the discussion is already about recovering the files, not about the editor's work.

What could NOT be quoted: discussions of deletion in the Resolve shared database.
The relevant threads (`forum.blackmagicdesign.com`, in particular `t=204958`,
`t=198254`, `t=113136`) do not open in this environment — Cloudflare returns 403;
the claim "Resolve deletes a media pool clip used in a timeline without a warning, and a request has been
filed for this" is taken from search results, is marked **[NOT VERIFIED]**, and bears no conclusion on its own.
The mechanics of the reference count, meanwhile, are taken from the primary source — the guide to the new
features of Resolve 18.6.

## How it fits Kinotrain

### What already exists

- **The guard exists, and it is complete within the document.** `Command::RemoveMedia`
  checks both clips and morpher substitutions
  (`p.clips.iter().any(|c| c.media == *id)`, `m.media_refs()`), and returns
  `CommandError::MediaInUse(id)` — `src/libs/kt-model/src/model.rs`. It is a gesture error, not a silent
  refusal (UI rule 16).
- **We do not delete the user's bytes at all.** Across all product code `std::fs::remove_file`
  occurs three times and never once on material: the autosave file, an unfinished export output file, and
  our own sidecar DB. That is, we have no "Media Files Are Deleted" command and there is no point in creating
  one — this is a strong position, and it is worth keeping deliberately, not by oversight.
- **The disappearance of material does not break the work.** An unavailable file receives a
  `PlaceholderSource` (a frame with the file name, silence), the project opens, the preview is complete,
  rendering proceeds (t209/t210), and a returned file is swapped in on the fly (`Player::replace_media`, t211).
- **Rendering honestly lists what is missing BEFORE the start.** `missing_sources`
  (`src/apps/kt-app/src/render_dialog.rs`) walks `project.media` and checks that the path exists; the list is
  shown in the settings window, and after completion — in the summary (`MISSING_AFTER`), so that an incomplete
  render does not look like a success.

### What is missing

1. **The list of missing items is taken at the wrong moment.** `missing_sources` is computed when the settings
   window is OPENED (`start_render`), while the sources are opened later, already in the render thread:
   `let (a, _) = open_all(…); let (v, _) = open_all(…)` — and both returned lists of missing items are
   THROWN AWAY, while the job is given `missing: self.render_setup.missing.clone()`, taken before the file-save
   dialog. A file that disappears between these moments silently becomes a placeholder: `compose_export_into`
   returns `complete == true` (the frame is there, after all), `stats.missing_video_layers` stays at zero, and
   the summary reports success. This is our own case of "the neighbor deleted it while I was pressing the button",
   and it is cured by one line — take the list from `open_all`, not from a check made half an hour earlier.
2. `open_all` is called TWICE (separately for audio and for video), so within one job the disk state is read in
   two steps: in theory the sound may come from the real file, and the picture from a placeholder.
3. A sign that "this material is needed by someone else" does not exist and cannot exist: a media item has
   neither an owner nor an external reference counter, and a `MediaId` is issued as "maximum + 1" and coincides
   for two people.
4. There is no log of "who removed it": undo consists of snapshots of the whole project
   (`undo: Vec<Project>`), and an edit has neither an author nor a name.

### What gets in the way

Nothing constructive — and this is a rare case. Item 1 from the list above requires
neither a new entity nor a new surface: `open_all` already returns exactly the list
that is needed, it is simply not taken. The other thing is more costly: telling the
user about a disappearance that happened DURING rendering is more honest than a line
in the final summary — but that is already a question of UI rule 19 (where to change
the decision later) and of wording, not of mechanics.

OFFLINE: REMAINS — `src/apps/kt-app/src/render_dialog.rs` — on one machine the question is closed completely and without any rights: only the user themselves could have deleted the file; the guard "material is in use" is exact, because the whole reference count is the whole document, and before rendering the program itself walks the project's material, names what is missing, and does not cancel the export because of it. Precisely for this reason the same guard lies for the neighbor: it honestly answers about my document and stays silent about someone else's.

NETWORK COST: to learn that the material is needed by a second person — access rights — a file in our world has neither an owner nor a sign of "busy", and media identifiers are issued as "maximum + 1" and coincide for two people, so there is nothing on which to hang someone else's reference.

NETWORK COST: to notice a disappearance at the moment of reading, not at the moment of checking — invariant 9 — the export is in-process and opens the sources itself, so the only one who sees the deletion in time is the render itself; and its answer is precisely what is thrown away today.

### Which invariants it touches

- **No. 9** (the export is entirely in-process): the one who opens the files is our
  own render thread, therefore fixing item 1 requires neither an external process
  nor a new queue: the list is already inside.
- The fix touches no other engine invariants: it concerns which of the two already
  existing lists gets into the job.
- UI rules: **16** (impossibility is an error: `MediaInUse` must remain a gesture
  error, not a faded hint), **19** (the dialog must say where the decision can be
  changed later — the summary of an incomplete render must say what to do with the
  placeholders).

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: the list of missing items is taken from `open_all`, not from a check before the dialog |
| Where the main work is | kt-app (`render_dialog.rs`: return the discarded list to the job and merge it with the pre-start one); kt-engine is not touched |
| Project format | not touched |
| Tests to write | a harness test: the file disappears between opening the settings window and the start of the job — the summary names it among the missing ones rather than reporting a clean success; a check that both `open_all` calls give a consistent list |
| Depends on | nothing; neighboring — F-HLP-012 (how to speak about a missing source), F-MED-005 (relinking: we can repair a path only by deleting the media, which `MediaInUse` forbids), F-RES-004 (what survives a crash) |
| Breaks compatibility | no |

## Verdict

The server half — rights, owners, "who deleted it" — we do not set up: there is no
participant, and without one it is not a mechanism but a field in a structure. The
formula is the same as for F-PRJ-003 and F-PRJ-001.

We take the local remainder found by the reconnaissance, which costs one stage: rendering
must report material that disappeared AFTER the check, not what was checked when the settings
window was opened. Today `open_all` computes the correct list twice and both times throws it
away, and the placeholder makes the frame "complete", so an incomplete render looks successful —
that is, exactly the scenario for which R.33 item 10 introduced the summary remains unclosed in its
most dangerous variant: when the file was removed by someone other than you.

The second thing this note fixes intentionally: **we have not and must not acquire a command that
deletes the user's material from disk.** The three calls of `remove_file` in product code touch only
our own files. The thread about the lost DJI sources is the price of the opposite decision, and it is
better not to pay it.

Boundaries: relinking and consolidation — F-MED-005; the message about a missing source — F-HLP-012;
distribution of derivatives — F-COL-005; a growing source — F-COL-004.

## Sources

- DaVinci Resolve 18.6 New Features Guide, "Removing Unused Media from a
  Project":
  https://documents.blackmagicdesign.com/SupportNotes/DaVinci_Resolve_18.6_New_Features_Guide.pdf
  (accessed 08.09.2026) — "This action loads all timelines, compound
  clips, and Fusion compositions, then analyzes them for media usage.",
  "This option does not delete any clips from your disk, only removes them from
  the Media Pool and the Project."
- Adobe Community, «Productions: source media has been deleted or moved»,
  03.07.2024 – 13.07.2024:
  https://community.adobe.com/t5/premiere-pro-discussions/productions-source-media-has-been-deleted-or-moved/td-p/14718332
  (08.09.2026) — the boundary of knowledge of closed projects, the recommendation
  "Reassociate Source Clips…".
- Adobe Community, «deleted media», 21.09.2024:
  https://community.adobe.com/t5/premiere-pro-discussions/deleted-media/td-p/14873391
  (08.09.2026) — "Media Files Remain on Disk" by default, the irreversibility of
  "Media Files Are Deleted".
- Apple Support, HT207128 (the shared Final Cut library and its exclusive lock):
  https://support.apple.com/en-us/HT207128 (08.09.2026) — the wording of the message about
  a busy library; requirements for SMB (Samba ≥ 4.3.4,
  `vfs objects = catia fruit streams_xattr`).
- Blackmagic Design, DaVinci Resolve — Collaboration:
  https://www.blackmagicdesign.com/products/davinciresolve/collaboration
  (08.09.2026) — "Bins and timelines are 'read only' until unlocked by the
  current user."; the page says nothing about rights to delete material.
- `forum.blackmagicdesign.com` (threads about deletion from the media pool) — UNAVAILABLE
  (403, Cloudflare); the corresponding claim is marked **[NOT VERIFIED]**.
- The VEGAS and After Effects rows are compiled from vendor documentation and the
  composition of menus; not checked live.
- Our code was read on 08.09.2026: `src/apps/kt-app/src/render_dialog.rs`
  (`missing_sources`, `open_all`, `launch_render`, the job summary),
  `src/libs/kt-engine/src/export.rs` (`missing_video_layers`, `complete`),
  `src/libs/kt-model/src/model.rs` (`Command::RemoveMedia`,
  `CommandError::MediaInUse`), greps for `remove_file` across the product code.
