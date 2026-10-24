---
id: F-PRJ-001
title: Several sequences (timelines) in one project
status: candidate
priority: B
cost: XL
category: project/structure
editors: [premiere-26, resolve-21, fcp-12, vegas-pro-2026, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-app, kt-cli, kt-engine]
invariants: [3, 4, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-PRJ-001 — Several sequences (timelines) in one project

## Essence

One project file holds SEVERAL independent edits and one shared bin
of material. The full version and the trailer, episode 1 and episode 2, the "rough cut" and the "fine cut", 16:9 and 9:16 — these are different sequences over the same
sources. Switching between them is a gesture inside the program, not "opening
another file": the material, the bin layout and the path links are shared, while the clips,
tracks, markers and the cursor are each their own. The assembly of the final cut from
parts also comes from here: ready pieces lie as sequences, and the master is assembled from them.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Sequence | there is as many sequences in a project as you like; each is an OBJECT of the project panel next to the material, so it can be put in a bin, renamed and dragged into another sequence ("each Sequence comes in as a large single clip"). Open sequences are timeline tabs, several are available at once. Each has its own settings (raster, timebase). The project is open EXACTLY ONE at a time: "you can only have one Project file open at a time", and for large works Adobe split sequences into DIFFERENT files — Productions |
| DaVinci Resolve | 21 (the device was checked against the 18 manual) | Timeline | a timeline is an element of the media pool: "Timelines can be organized like any other clip in the Media Pool"; it lies in a bin, is duplicated with the Duplicate Timeline command, there is a Smart Bin for timelines and a timeline ON/OFF switch. Format settings are project defaults by default, but a timeline can have its own. One level higher is not a file but a DB: "A Project Library (formerly project database), is a database file storing one or more DaVinci Resolve projects" |
| Final Cut Pro | 12.3 | Project (inside an Event inside a Library) | for them the "project" IS a sequence: the hierarchy Library → Event → Project, as many projects as you like in an event. Versions are made by duplicating, and there are TWO duplications: the ordinary one and "as Snapshot", which embeds copies of compound/multicam parent clips |
| VEGAS Pro | 2026.0 (help checked against the branch 22 — HTML help for 2026 is not publicly available) | none | a project = ONE timeline (`.veg`). Something similar is done by nesting, but a nest is a separate file: "Nested timelines are technically just nested projects, but with a big improvement: To edit it, you don't have to open it in a separate instance of VEGAS Pro" |
| After Effects | 26.x | Composition | as many compositions in a project as you like, each with its own settings (raster, fps, duration) and its own tab; but AE is not an NLE, and "an edit version" is not expressed there that way [NOT VERIFIED against the primary source: `helpx.adobe.com` does not open from this environment, see "Sources"] |

## The trick behind it

**1. A sequence is not a document but an ELEMENT OF THE BIN, and everything else follows from this.** The Resolve manual says it directly: "Timelines can be organized like
any other clip in the Media Pool". Since a timeline lies in the bin on a par with
files, it gets for free a name, a folder, search ("Smart Bin for Timelines"),
duplication (Duplicate Timeline) — and the ability to be dragged into another
sequence, that is, NESTING (F-TL-004) turns out not to be a separate feature, but
a side effect. The reverse side of the same design: a copy of a sequence
copies REFERENCES, not content, so "I made a copy before the edit" is
not a guarantee. FCP had to introduce a second command for this: "When you
duplicate a project as a snapshot, Final Cut Pro embeds copies of compound or
multicam “parent” clips in the duplicate, so any changes to other instances
of those clips do not affect the duplicate". That is, they have TWO different
"duplicate" operations, and the editor must know how they differ — otherwise a version
made "just in case" travels along with the original.

**2. A sequence's own settings are not a convenience but a way to release
different masters, and they are also the source of silent surprises when moving clips.**
Resolve states the purpose honestly: "you have the option of creating
separate timelines with individual Format, Monitoring, Output Sizing, and
Color Management settings for situations where you need to set up multiple
timelines to create multiple deliverables with different resolutions, pixel
aspect ratios, frame rates". This is exactly where 16:9 and 9:16 from one
material come from. The price: a clip's transform is stored as a NUMBER in the coordinate system
of ITS OWN sequence (scale is a percentage of its raster), so when copying between
sequences with different settings the number moves, but the meaning does not. Oliver
Peters on Creative COW: "When I copy-and-paste clips between sequences, this
type of sizing information is lost. Generally position information is
retained, but the scale value is wrong". And what finishes it off is not the raster difference
itself but the IMPORT SETTING: according to an observation in the same thread, with auto-fit to frame enabled "motion settings don't get included in pastes" — that is,
the paste behavior depends on a preference that the editor set half a year ago and that has nothing to do with sequences.

**3. Both flagships ran into the PRICE of "many edits in one file" — and both created a separate mechanism to avoid paying it.** Resolve added a timeline switch and explained why: "Since having a large number of
timelines within a single project file can affect performance, having the
ability to disable timelines lets you maintain these backup/alternate
timelines without any penalty" — disabled ones are not loaded into memory and do not
affect the speed of opening, saving and export. Adobe went the other way: Productions spread sequences across DIFFERENT files, "keeping projects
small and responsive", because with one file "the RAM usage grows with
the project size. The larger the project gets, the longer it takes to open
and save, too". The conclusion that is in none of the feature lists: "many
sequences in one project" is not a free organizational luxury but a compromise, for which both editors pay with a separate mechanism. For
us this is a direct figure: undo is `Vec<Project>` of 80 full snapshots, and
sequences multiply it in width.

## Pros

- One bin: relinking a missing file or replacing a source fixes ALL edits at once,
  not one per file. For us this is exactly what "source return" (t211) already does — but today it fixes one timeline,
  because there is only one.
- An edit version stops being a file manager's job: "duplicate the sequence" is a command inside the program, the copy immediately has a name and a place, and
  it does not need to be searched for among `film_final_final2.otio`.
- Different masters from one material are expressed without export and re-import:
  a sequence in 9:16 takes the same clips, not a rendered file.
- Assembling the final cut from parts comes out by itself where a sequence can
  lie inside another sequence: "episodes" are ordinary objects of the bin.
- The cache and the index do not suffer from this: they are keyed by MEDIA, not by the edit,
  so warmth and the peak pyramid are reused between sequences for free.
- The discipline "we do not touch the draft" becomes checkable: the old sequence
  lies next to it and opens in one click, rather than being restored from a backup.

## Cons

- The file grows, and EVERY operation on the document pays for it: opening,
  saving, autosave and the undo snapshot cost as much as the whole set of sequences, even
  when only one is being edited. Resolve answers this with a timeline switch,
  Adobe — by splitting into files; both are an admission of the problem.
- A copy of a sequence is not isolation: shared nests/compound objects keep
  changing under the copy until a "snapshot" is made. The class of errors "I made a new
  version — the old one moved" exists precisely because of this.
- Moving clips between sequences with different settings is recalculated silently, and both sides show "100 %" — that is, it looks like healthy
  work (see the quote above).
- The question "what exactly belongs to a sequence" appears: settings, markers, areas, track order, region, zoom and cursor — each editor has its own
  dividing line, it is not written down anywhere explicitly, and it is learned in practice.
- Undo. History is either shared (then a rollback "in the wrong" sequence is frightening), or
  separate (then operations touching two sequences at once cannot be rolled back in one
  step). There is no third option, and both variants are unpleasant.
- Export stops being unambiguous: "render the project" no longer means anything until it is said WHICH sequence — and this breaks every
  command line and every golden test written before the feature.

## What users say

The very formulation of the question "why sequences if there are projects" is the thread that everyone starts with:

> Why use sequences? Why not just build a project?
> — Corinne Friesen, Creative COW, 31.03.2010, https://creativecow.net/forums/thread/sequencesae-why-not-just-build-projectsae/

> A SEQUENCE is the timeline (inside the Project) where you assemble the
> clips together and do the editing.
> — Jeff Pulera, Creative COW, 02.04.2010, https://creativecow.net/forums/thread/sequencesae-why-not-just-build-projectsae/

> If you are doing a feature length project and you have over 1,000 clips,
> it can help to break the project into scenes by using different sequences.
> — Vincent Rosati, Creative COW, 31.03.2010, https://creativecow.net/forums/thread/sequencesae-why-not-just-build-projectsae/

The negative one is about moving between sequences:

> When I copy-and-paste clips between sequences, this type of sizing
> information is lost. Generally position information is retained, but the
> scale value is wrong.
> — Oliver Peters, Creative COW, 11.11.2017, https://creativecow.net/forums/thread/scaling-copy-bug/

> If the setting is turned off, paste works correctly. If it is on, motion
> settings don't get included in pastes.
> — John Pale, Creative COW, 12.11.2017, https://creativecow.net/forums/thread/scaling-copy-bug/

And about WHY sequences became the unit of work rather than projects:

> One of the limitations of Adobe Premiere is that you can only have one
> Project file open at a time.
> — Larry Jordan, 2015, https://larryjordan.com/articles/premiere-pro-cc-share-sequences-between-projects/

What could not be found: live forum complaints about the "project = a record in a
database" model in Resolve. `forum.blackmagicdesign.com` returns 403 on any
`viewtopic.php` from this environment, reddit is unavailable, and search results for the
query "resolve database vs project file" lead either there or to Facebook groups and tutorial sites. Therefore the section on the DB relies on
official manuals, not on feedback.

## How it fits Kinotrain

### What already exists

- **A project = a timeline, literally.** `struct Project` in
  `src/libs/kt-model/src/project.rs` is a flat structure, where in one
  list of fields lie the material (`media`), the edit (`clips`,
  `transitions`, `morphers`, `cameras`, `markers`, `areas`, `lane_order`,
  `*_lanes`), the view state (`ui: Option<UiState>`), and the transport
  (`region`, `loop_enabled`).
- **The shared bin is already almost free.** `MediaRef { id, path }` knows nothing about
  clips; the sidecar index `<project>.ktindex.db`
  (`src/libs/kt-media/src/index.rs`) is keyed by files; `BurstCache`
  (`src/libs/kt-cache/`) — media + quality + range. That is, half of the promise
  "one material, many edits" is already built correctly by us.
  This is also in the format: `metadata.kt.bin_media` — "media NOT on the timeline"
  (`docs/file-format.md`).
- **The engine is indifferent to sequences.** `Player::set_project(p: Project)` in
  `src/libs/kt-engine/src/player.rs:315` receives a FLAT project; if
  kt-app hands over the active sequence, kt-engine/kt-gpu/kt-cache need not be touched at all. The feature does not touch invariants 5–9 and 12.
- **The format fork has already been decided — and decided AGAINST.** `docs/file-format.md`,
  section "v1 limitations (documented, not implemented)":
  "`SerializableCollection` root (several timelines) — refusal".
  The same in `src/libs/kt-otio/CLAUDE.md`: "SerializableCollection root →
  error". This is not a guess: in `src/libs/kt-otio/tests/format.rs` (test 146)
  a collection root is directly checked as `Err(Error::UnsupportedRoot(_))`,
  and item 146 records "A root that is not a Timeline
  (SerializableCollection) → refusal". The feature means reversing this decision.
- **Undo — full snapshots.** `TimelineModel { project, undo: Vec<Project>,
  redo: Vec<Project> }` and `MAX_UNDO = 80` in
  `src/libs/kt-model/src/model.rs`; `push_undo` puts in `self.project.clone()`.

### What is missing

The concept of a "sequence" in the model and in the format; a choice of the active sequence in the UI and
in the CLI; a per-sequence `Retained` for passthrough; an answer to the question of whose
undo history it is.

### Splitting `Project` — the most useful part of the analysis

An analysis of the fields of `src/libs/kt-model/src/project.rs` by ownership:

| Field | Who owns it | Remark |
|---|---|---|
| `format_version` | the document | one version per file, as now |
| `media: Vec<MediaRef>` | the DOCUMENT | this is the shared bin — everything exists for its sake |
| `UiState.bin_free` / `bin_layout` / `bin_groups` | the DOCUMENT | the bin layout is one for the whole file |
| `settings: ProjectSettings` | disputed | in Resolve this is a document default that a timeline CAN override; for us `fps` enters the frame grid and the burst step, and `width/height` enter the render raster. The fork "inherit or own" is the same as that described in F-TL-004 for nests, and it must be decided ONCE for both |
| `clips`, `transitions`, `foreign` | sequences | |
| `video_lanes`, `audio_lanes`, `morph_lanes`, `camera_lanes` | sequences | |
| `lane_order` | sequences | since R.35 it is IN THE MODEL and goes through undo (a morpher acts "on everything below"), so it is part of the edit, not of the view |
| `morphers`, `cameras` | sequences | |
| `markers`, `areas` | sequences | and together with them — a COMMON namespace (`validate()` keeps one `names` for range markers and areas). It becomes per-sequence, and the morpher's reference `MorphTargets::Areas{names}` resolves inside its own sequence. This must be said aloud: otherwise "area final" in two sequences is a trap of exactly the kind for which the namespace was merged (R.35 item 2) |
| `region`, `loop_enabled` | sequences | today they are outside undo and are carried through `undo()`/`redo()` by hand — when switching sequences they will have to be carried together with the sequence, otherwise the loop will "stick" to someone else's edit |
| `timeline_end` | sequences | the edit length is a property of the edit |
| `overlap_layers` | the DOCUMENT (by the decision of R.28) | the user asked specifically for a "project option" — it cannot be changed without a new request, although it changes the legality of geometry on tracks, that is, in meaning it is a property of the sequence. This is a constraint, not a choice |
| `UiState.view_start` / `ticks_per_px` / `cursor` / `lane_names` | sequences | each edit has its own zoom, its own cursor and its own track names |
| `UiState.meters_w` | the document | the width of the meters panel has nothing to do with the edit |

So `UiState` falls apart along exactly the same line as `Project`:
the bin part — to the document, the viewport part — to the sequence. A useful side
result: today these seven fields lie in one structure only because
"everything outside undo" was dumped into one heap.

### What gets in the way

1. **Undo: the snapshot and the commonality of history.** `push_undo` clones the WHOLE
   `Project`; with N sequences every step of any edit copies all N
   edits, and the depth of 80 is fixed. The fork (which the owner, not the programmer, will have to decide): history COMMON to the document — then a rollback
   after switching sequences returns an edit to an invisible-right-now
   edit; history SEPARATE for each — then operations touching two at once
   (pasting a block from sequence A into B; a `RemoveMedia` that both hold)
   cannot be rolled back in one step, while our batch is atomic by construction
   (`apply_batch` → `simulate` → `validate`). A third option — a common
   history, but the snapshot stores only what changed — exists, but this is already design, and it is possible ONLY after the structure is split: while
   `Project` is flat, there is nothing with which to make the snapshot cheaper.
2. **`RemoveMedia` — exactly the place where the shared bin breaks.**
   `check_on` in `src/libs/kt-model/src/model.rs` (the `Command::RemoveMedia` branch)
   looks at `p.clips` and `p.morphers` of ONE project.
   With several sequences the guard must review all of them, otherwise deleting a card
   from the bin will quietly break a neighboring edit — and will break it invisibly,
   because a different one is open.
3. **`ClipId` and the key of FX state (invariant No. 3).** The uniqueness of `ClipId`
   is checked by `validate()` within a `Project`; the high bit is already given to the pieces
   of the morpher layer (`SLICE_TAG`, `MAX_REAL_CLIP_ID`), and FX state in the engine
   is addressed by `ClipId`. Either the ids remain cross-cutting across the whole document
   (simple, breaks nothing, but the id counter is a property of the document, not of the
   sequence), or they become per-sequence — and then the key of FX state and `Retained.clips` must become a
   pair (sequence, clip), otherwise the reverb tails of two different edits will collapse into one state.
4. **Format: `Retained` is built as ONE timeline.**
   `src/libs/kt-otio/src/convert.rs` — `Retained { timeline_name,
   stack_name, global_start_time, tracks: HashMap<(ClipKind,u32),…>,
   clips: HashMap<ClipId,…>, transitions, foreign, … }`. The passthrough of a foreign
   file will have to be kept per-timeline, otherwise foreign metadata will get glued
   between sequences. Plus the envelope itself moves: `bin_media`,
   `bin_groups`, `bin_layout` today lie in `Timeline.metadata.kt`, and they must be on the root collection.
5. **Byte stability of a file with ONE sequence.** Our golden files
   (`src/libs/kt-otio/tests/format.rs` t144, t148 and
   `src/fixtures/demo-legacy-import.otio`) rest on the root being a
   `Timeline`. So a document with one sequence must continue to be
   written as a single `Timeline`, and `SerializableCollection` appears only with the second.
   Otherwise the reference files would have to be regenerated, and that cannot be done without the owner's request.
6. **Test 146 is a regression test.** It asserts that a collection root is rejected. The feature makes this assertion false, and the tests 1–221 "must not be weakened or deleted without an explicit decision by the user" (the root
   `CLAUDE.md`). That is, the feature runs not into the code but into the cancellation of an already adopted decision — and the note must name this, not go around it.
7. **UI: 133 calls to `model.project()` in `src/apps/kt-app/src`.** Each means "the current edit". If `TimelineModel` continues to hand out the active sequence, almost all will survive without edits — but this must be ensured deliberately, not hoped for.
8. **`model_rev` — one counter per document.** Caches of coverage polls, clip names and stretch maps are keyed by it
   (`src/apps/kt-app/src/app.rs`, fields next to `model_rev`). An edit in sequence B will invalidate the caches of sequence A. Not a defect, but
   free work on every switch, and it must be counted in advance.
9. **Autosave and the write guard.** `write_project_guarded`
   (`src/apps/kt-app/src/app.rs`) compares the revision of the snapshot with the one already on disk — the race does not get more complicated, the file is still one and is written
   atomically (`write_project_atomic`). But the COST of background autosave
   grows linearly with the number of sequences, because the whole document is serialized; this is exactly what the Resolve manual warns about, and exactly
   what Adobe moved away from with Productions.
10. **Export and CLI (invariant No. 11).** `kt-engine::export` and
    `src/apps/kt-cli` take the project as a whole; "render the project"
    stops being unambiguous. A choice of sequence is needed — in
    `src/apps/kt-app/src/render_dialog.rs`, in the default file name
    (t201) and as a CLI argument; the golden tests (`golden_export.rs`, t11/t67)
    will have to be bound to a named sequence.

### Which invariants it touches

No. 3 — the key of FX state (`ClipId`) ceases to be unique by itself
if ids are numbered within a sequence. No. 4 — "the cache is the single truth about
coverage" is touched in a GOOD direction, and this must be recorded explicitly:
warmth is common to all sequences, because the key is the media, and no
second bookkeeping "the warmth of this sequence" may be started. No. 11 — the
determinism of export requires that it be said WHAT is exported. UI rules: 13 (a permanent row of tabs for an object of which there is usually one — a direct "waste of space"; in Resolve the tabs are not made an
OPTION for nothing, and the default is a double-click on a bin element), 17 (switching from the bin and from the menu — already two ways, a third requires justification), 20
(a sequence is not created by itself; an old file opens as a document with exactly
one).

## Estimate

| What | Answer |
|---|---|
| Scope | XL — a round plus a change of format and cancellation of a previously adopted decision about the OTIO root |
| Where the main work is | kt-model (splitting `Project`/sequence, `validate`, command guards, undo), kt-otio (`SerializableCollection` for reading and writing, `Retained` per-timeline, moving the bin part of the envelope), kt-app (active sequence, switching, splitting `UiState`), kt-cli + render_dialog (choice of sequence). kt-engine/kt-gpu/kt-cache — not touched |
| Project format | yes: the root becomes `SerializableCollection` with two or more sequences (with one — the former `Timeline`, otherwise t144/t148 will shift), `kt.v` is raised, the bin keys move to the root |
| Tests that will have to be written | a model round trip with N sequences and switching; byte stability of a file with one sequence (t144/t148 without regeneration); REWRITING t146 (a collection root stops being an error) — only on an explicit decision by the owner; `RemoveMedia` through all sequences; undo across a switch; export of a named sequence in kt-cli and golden files on it; a layout test of the switcher (rule 13) |
| Depends on | nothing. It overlaps with F-TL-004: "a sequence inside a sequence" is nesting, and the fork "inherit the parent's settings or have one's own" is COMMON to them; it cannot be decided twice |
| Breaks compatibility | our files are read as a document with one sequence; the behavior of importing foreign `SerializableCollection` files changes incompatibly (today — an honest refusal) |

## Verdict

As a FEATURE — we do not take it now. Over all the rounds of `docs/user-feedback.md` (R.4–R.37)
there is not a single mention of a second sequence, edit versions, episodes or
layouts for platforms: the user asked for transitions, automation,
the morpher layer, slow motion, copy-paste, area selection — all of these are operations WITHIN
one timeline. The workaround is honest and cheap: "Save As" and a second file;
the shared bin is lost, but for us it is already almost free (the index and cache are keyed by
media, not by the edit). And the main argument against panic: our UX reference is
VEGAS, and VEGAS has none of this at all — the project there is still one timeline.
Priority B, not A, for exactly this reason.

But this is the only position of the base whose price grows from POSTPONEMENT, not from the amount of work. Each new field in `Project` (over the last three rounds
five were added: `lane_order`, `areas`, `cameras`, `camera_lanes`,
`overlap_layers`) and each new call to `model.project()` (today 133
in kt-app) make the split more expensive, and the edit itself gains nothing from this. Therefore the recommendation is not "make sequences," but at the nearest change of format, which raises `kt.v` anyway, to name in the model the boundary:
what belongs to the document (material, bin, the write guard), and what belongs to the edit
(clips, tracks, markers, areas, region, viewport). Beyond that the feature itself is the
second element of the list and the switcher, that is, M, not XL. Separately, before any attempt,
two decisions by the owner are needed: whether a sequence inherits the document's settings (the common fork with F-TL-004) and whether test 146 is rewritten.

## Sources

- https://creativecow.net/forums/thread/sequencesae-why-not-just-build-projectsae/ (accessed 2026-09-07)
- https://creativecow.net/forums/thread/scaling-copy-bug/ (accessed 2026-09-07)
- https://larryjordan.com/articles/premiere-pro-cc-share-sequences-between-projects/ (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/duplicate-projects-and-clips-verfd45ffa45/mac (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-libraries-verb9d7e0dbc/mac (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/nested%20timelines.htm (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/3-projects/project_nesting.htm (accessed 2026-09-07)
- DaVinci Resolve 18 Reference Manual, ch. 5 "Edit" (sections "Switching Among
  Multiple Timelines", "Individual Timeline Settings…", "Duplicating
  Timelines", "Disabling Timelines") and ch. 16 "Project Libraries…" (section
  "What is a Project Library?"). The original is
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_18_Reference_Manual.pdf;
  a page-by-page mirror was used:
  https://ltbits.github.io/davinci-resolve-manuals/ (accessed
  2026-09-07). The wording about "Do not, under any circumstances, remove,
  rename, or attempt to edit this file" — from the DR12 manual, ch. 41, also on the mirror; in 18/21 the section was rewritten for "Project Library", so this quote is not used in the text of the note.
- https://blog.frame.io/2024/09/30/ultimate-guide-to-premiere-pro-productions/ (accessed 2026-09-07) — a secondary source (the Frame.io blog, an Adobe product), taken for the wording about the cost of a large `.prproj`
- Code and documentation of the project: `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-model/src/model.rs`, `src/libs/kt-otio/src/convert.rs`,
  `src/libs/kt-otio/tests/format.rs`, `src/libs/kt-otio/CLAUDE.md`,
  `src/libs/kt-engine/src/player.rs`, `src/libs/kt-media/src/index.rs`,
  `src/apps/kt-app/src/{app.rs,bin_panel.rs,render_dialog.rs}`,
  `docs/file-format.md`, item 146,
  `docs/user-feedback.md` (R.28, R.35).

What could not be verified. The Adobe help (`helpx.adobe.com`) and reddit do not open from
this environment, `forum.blackmagicdesign.com` returns 403 on
`viewtopic.php`, and the search engines Brave/DuckDuckGo/Mojeek return 429/403/CAPTCHA;
Yahoo and Bing worked. Therefore the Premiere row is assembled from Creative COW and
larryjordan.com, the After Effects row is marked `[NOT VERIFIED]`, and
the Resolve device is taken from the official manual for 18 (not 21): it was not possible to check live whether anything changed in 20/21 — the version in the header is
`resolve-21` with a caveat in the "Who has it and how" table.
