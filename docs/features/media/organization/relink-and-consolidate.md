---
id: F-MED-005
title: Relinking sources and collecting a project into one folder
status: candidate
priority: B
cost: L
category: media/organization
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-media, kt-engine, kt-app, kt-cli]
invariants: [4, 5, 8, 9, 11]
ui_rules: [12, 16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-EXP-003]
trackers: [kde-bugzilla, kde-invent, pitivi, olive, flowblade, shotcut, openshot]
checked: 2026-09-08
---

# F-MED-005 — Relinking sources and collecting a project into one folder

## Essence

The files have moved — to another disk, to another folder, to another machine. The edit
is intact, but it looks into emptiness. The program needs to be told where the material now lies,
and this has to be done NOT one file at a time out of three hundred. The reverse task of the same
pair: hand the project to another person so that everything opens for them — that is,
gather the material scattered across disks into one folder and rewrite
the links to it.

Boundaries. About HOW the program reports a missing file and what it shows
instead — F-HLP-012 (`help/errors/offline-media-messaging.md`); about
what the engine does when the format is absent on this machine — F-RES-009
(`resilience/environment/codec-and-format-availability.md`). Here — the repair itself and the collection itself.

## Who has it and how

The row describes the COLLECTION (relinking across all five is analyzed in F-HLP-012 and is not repeated; only what the collection ends with is taken from it here).

| Editor | Version | Name | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Save As → **Copy media with project** | a checkbox in the ordinary save dialog, then a choice of two: `Copy source media` (copy whole) or `Create trimmed copies of source media` with the field `Extra head and tail`. Trimming is a real render into new files, and only for DV AVI / uncompressed AVI / YUV AVI; everything else "will not be trimmed, but will simply be copied". It ends with the CURRENT project being edited: events are switched to the new files, inactive duplicates are deleted, the project is saved. Material already lying BELOW the project folder is not copied |
| Adobe Premiere | 26.x | **Project Manager** | one window, two incompatible operations: `Collect Files and Copy to New Location` (a copy of the project + all material in one folder) and `Create New Trimmed Project` (a new .prproj referencing only what is used). Options: `Exclude Unused Clips`, `Include Handles` (number of frames before and after), `Make Offline`, `Rename Media Files To Match Clip Names`, `Include Preview Files`. The manual itself warns: a long GOP is not trimmed, multicam is not trimmed, "Project Manager does not carry all effects into a trimmed project" |
| DaVinci Resolve | 21 | **File > Media Management** | the most complete implementation. Scope — `Entire Project` / `Timelines` / `Clips`; operation — `Copy` / `Transcode` (in the section on trimming the manual also mentions `Move`); mode — `All media` / `Used media` / `Used media and trim keeping x frame handles`. Separately: `Consolidate multiple edit segments into one media file`, `Preserve folder hierarchy after x folder levels` (default 0), `Use project name subfolder`, `Relink to new files`. The result is a new `.drt` next to the material. Trimming works also for temporal codecs (H.264/XAVC/AVC-Intra) without re-encoding |
| Final Cut Pro | 12.3 | **File > Consolidate [item] Media** | there is no collection "into a folder" — there is collection INTO THE LIBRARY: the files are pulled into the library's storage location. The direction determines the operation: outward from the library — MOVE, inward or between external folders — COPY ("These rules prevent broken links from other libraries"). Scope — library / event / project / clip; a choice of types (original, optimized, proxy). No trimming at all. A separate command for Motion content |
| After Effects | 26.x | **File > Dependencies > Collect Files** | a new folder, a copy of the project, copies of the sources and a report. Scope — `All` / `For All Comps` / `For Selected Comps` / `For Queued Comps` / `None (Project Only)`; plus `Reduce Project` (throw out unused items), `Obey Proxy Settings`, `Generate Report Only`. No trimming. The report lists "files, effects, and fonts necessary to re-create the project" — the only one of the five that counts as a dependency not only media files |

A second table (software outside the core) is absent here: all five core rows are filled
with a mechanism, "The trick behind it" is taken entirely from their manuals, and by the criteria
of `_editors.md` a sixth row would be filler. The open editors take part in the note as a source of count and quotes (`_trackers.md`), not as a comparison row —
as the thesis of the third wave requires.

## The trick behind it

**1. A relative path is the only thing by which two `C0001.MP4` differ from each other, and both halves of the domain throw it out.** Relinking
compares the BASE NAME (Flowblade — literally `basename` in python), and collection
puts everything into one flat folder. Taken separately, each decision looks
harmless; together they give an irrecoverable loss, and this is visible in the tracker
in the correct order of cause and effect: first Kdenlive gets
"Archive Project bungs all clips into the same directory" (#447827,
02.01.2022), then — "Missing files in project archive if project contains
multiple videos with same filename" (#456346, 05.07.2022, FIXED). A name collision
from two memory cards is not a separate defect of collection, but a CONSEQUENCE of
the discarded folder structure. Resolve alone made of this an explicit
value: `Preserve folder hierarchy after x folder levels`, and by default there
it is **0** — that is, flatten. Its second half solves a neighboring,
narrower collision (two pieces of ONE file) and solves it differently — with the suffix
`_S000`, `_S001`, `_0`, `_1` according to the format. Two different collisions, two different
mechanisms, and people confuse them constantly.

**2. "Trim to what is used" almost nowhere works on what people shoot with today — and stays silent about it.** Vegas trims only
intra-frame AVI, the rest "will not be trimmed, but will simply be
copied"; Premiere: "Certain Long GOP formats cannot be trimmed by the
Project Manager", and multicam sequences are not trimmed at all. The button
is pressed, progress runs, and the output is the same gigabytes. Resolve 21 is
the only one that fixed this, and writes about it in a separate paragraph: trimming
is now compatible with temporal codecs "without recompressing or
transcoding". That is, the real cost of collection with trimming is not "you lose the margin beyond the edges", but "the function either fails or re-encodes", and
the difference between these two outcomes is not shown to the user.

**3. Trimming breaks not the frames, but the addressing in time of the material.** Premiere
in the same section of the manual: "Project Manager retains any effect keyframes and
clip markers that exist beyond the In and Out points of a trimmed clip" —
effect keys outlive the material they are bound to, and end up beyond the edge of the new file. Right next to it: "Project Manager does not carry all
effects into a trimmed project. Check your trimmed project carefully before
discarding the source project." The vendor says in its documentation that
the result must be re-checked by eye before you delete the originals —
that is why people fear collection with trimming, not because of handles. Vegas, meanwhile,
does all the same thing OVER THE CURRENT project: "All events are then updated
to point to the new files. Finally, inactive takes are removed from the
project, and the project is saved" — the only operation of the five
that irreversibly edits the open document rather than creating a copy.

## Pros

- Collection ends with relinking, not beside it: in Resolve this is the checkbox
  `Relink to new files`, in Vegas — "All events are then updated to point to
  the new files", in Premiere's trimmed project — the rewritten references in the new
  project file. That is, the mechanism is ONE: "rewrite the paths in bulk", while
  copying the files is a prefix in front. Whoever did the relinking
  did 70 % of the collection.
- The batch hypothesis is cheaper than a one-by-one dialog by two orders of magnitude, and this is
  measured out loud: Flowblade #929 — 313 files after a disk change versus
  four instructions after the fix; OpenShot #1829 — "it's enough to point to
  the first file".
- The scope of the operation as a separate value (Resolve: `Entire Project` /
  `Timelines` / `Clips`; AE: `For Selected Comps` / `For Queued Comps`)
  turns one command into three different scenarios — hand over the edit,
  give away one episode, clean out the bin — without three menu items.
- At FCP the direction is derived from the storage location, not asked of the person
  (outward — move, inward — copy), and the rule has a named reason:
  do not break the links of OTHER libraries to the same file. The only wording among the five
  that accounts for the file possibly being shared.
- The AE report (`Generate Report Only`, without copying) — a collection used as a completeness check. A neighboring feature: F-HLP-011 analyzes it as a preflight.

## Cons

- A flat folder on output (the default in Resolve; in Kdenlive — the only behavior until #447827) is an irreversible loss of the information that
  distinguishes same-named files from different cards. The structure cannot be
  recovered from anywhere.
- Trimming is not reversible. A trimmed project in Premiere is a new file, but
  the originals get deleted after it (that is the point), and "retains keyframes
  beyond the In and Out points" becomes irreversible already by the user's own hands. Vegas edits the current project directly in place.
- Collection silently changes the properties of the material: Vegas, when trimming, re-muxes the video into AVI and the audio into Wave/Wave64; Premiere, with the checkbox
  `Rename Media Files To Match Clip Names`, renames the files. Neither is rolled back.
- The dependency is defined at all of them as "a media file", and this is not the whole truth. AE
  counts fonts (only in the report, not in the copy); Kdenlive got three separate tasks on this:
  fonts are not packed (#501914), the `Counter` generator is not packed (#420623), the built-in luma masks are not packed (#492435). Everything that is not a file on disk drops out of the collection.
- Collection without relinking is useless, and this is caught directly in the tracker: Pitivi's
  "Export as Archive" puts relative paths into the archive, but leaves absolute ones in the
  `.xges` itself (#2050) — the unpacked archive does not open for anyone except its author.
- Relinking by name breaks exactly where names are not unique, and relinking by CONTENT is not done by any of the five: not one manual mentions a hash or size as a match criterion. Vegas
  honestly calls the result a guess (`Probable Match`), FCP, on the contrary,
  requires a match of type, fps and channels, but compares the name and path.

## What users say

About the cost of one-by-one relinking — from a person who fixed it:

> I replace a hard disk and I had to relink 313 files. 313 selections to do
> because you can only select file by file!
> — Flowblade #929, 09.11.2020, https://github.com/jliljebl/flowblade/issues/929

There, the result of the fix: "Now when I relink a file, Flowblade search in the
directory of the relink file if other files have the same names (basename in
python)… In my case, only 4 selections were enough." The word `basename` is verbatim from the ticket, and it is exactly the mechanism that breaks on duplicates.

The same request independently in OpenShot, also from the author of the patch:

> OpenShot got stuck in a loop asking where the media files are. I've fixed
> this problem by auto-linking all files in the same folder, so that it's
> enough to point to the first file.
> — OpenShot #1829, 10.07.2018, https://github.com/OpenShot/openshot-qt/issues/1829

And in Olive — from a user, not a developer:

> I just reorganized a bunch of my files and it unlinked almost all the media
> in my project, so I had to relink it all one by one.
> — Olive #610, 06.03.2019, https://github.com/olive-editor/olive/issues/610

About a collection that did not rewrite the paths:

> Currently the .xges file in the archive contains the absolute paths of the
> original files (assets), while the archive itself contains paths under
> projectname-export/ relative to the creating user's home directory.
> — Pitivi #2050, 11.07.2017, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2050

About trimming that did not trim (Vegas and Premiere, the same complaint
ten years apart):

> When you 'Save As' and click on 'Copy media with project' – you are
> supposed to get 'trimmed' copies of the source media – so you can get rid
> of all that video you don't need. But it doesn't work over here in Texas.
> — Marius Perron, 08.09.2013, https://creativecow.net/forums/thread/copy-media-with-project-does-not-trim-the-source/

> When I try to use Project Manager to take the clips from a completed
> Sequence and transcode the use portions plus handles the system just
> exports the entire clip in full.
> — aizickg14671823, 28.12.2023, https://community.adobe.com/t5/premiere-pro-discussions/project-manager-not-reducing-clips-and-adding-handles/td-p/14322787

The reply in the same thread names the cause, and it is the same as in the Vegas manual:
"One notable limitation of Project Manager, is that they can't 'cut' any
H.264/5 long-GOP media down, because PM can't sort out where the iframes
are" (R Neil Haugen, 28.12.2023, same thread).

And negative feedback from the other side — about relinking that cannot end at all in principle:

> I would like the option to ignore those files and just enter them as blank
> placeholders, and I would also like the option to re-render these
> placeholders at a later stage.
> — Flowblade #958, 15.01.2021, https://github.com/jliljebl/flowblade/issues/958

SIGNAL: collecting a project into one folder — 15 tasks in kde-bugzilla, `collect files|consolidat\w*.{0,15}(media|file|project|clip)|archive.{0,15}project|project.{0,15}archive|package.{0,15}project`; share 0.2% against 0.0% (1 of 2672) in pitivi

SIGNAL: relinking as a dialog — 8 tasks in pitivi, `relink|re-link|reconnect.{0,20}(media|clip|footage|file)|(locate|find|search)\w*.{0,25}(missing|moved|offline).{0,25}(file|clip|media|footage)|missing.{0,25}(file|clip|media|footage).{0,25}(dialog|search|locate|relink)`; share 0.3% against 0.0% (3 of 6346) in kde-bugzilla

SIGNAL: "pointed at one — all were found" — 1 task in olive, `relink.{0,30}(director|folder|\ball\b)|(all|other|remaining|rest).{0,25}(clip|file|media|footage)s?.{0,25}(same|that|new).{0,15}(folder|director|path|location)|automatic\w*.{0,30}(relink|find|locate).{0,30}(moved|missing|file)`; share 0.1% against 0.0% (1 of 6346) in kde-bugzilla

What lies behind these numbers.

**The first pair — about where the pain of the domain lives.** "Collect project" exists
at exactly one of the nine open editors — at Kdenlive ("Archive
Project") — and it accounts for 15 tasks, laid out without remainder:
four crashes (#418301, #420494, #474792, #378576), three about wrong paths
in the collected project (#377999, #427162, #447827) plus #466561 about moving
a directory, three about the fact that a dependency is not counted as a dependency (fonts
#501914, the `Counter` generator #420623, the built-in luma #492435), three
requests about the scope of the operation ("add clips in timeline" #383578, "archive
only selected items" #505540, "exclude proxy files" #492763) and, on a separate
line, #456346 — same-named files. In kde-invent these are joined by
#535 (paths), #561 (scope) and #999. This is the most defect-dense function
of the whole domain: 0.2 % of the tracker for
ONE menu command. Pitivi has the same function ("Export as Archive"), giving exactly
one task — but this task (#2050) says that the unpacked archive does not
open, that is, the function does not work for its purpose as a whole. The conclusion for
us: what is expensive here is not the copying, but the rewriting of paths, and the whole cost is in it.

**The second pair — about form.** Eight Pitivi tasks are not eight different
problems, but eight complaints about ONE modal "Missing files
dialog" when opening a project: the window size (#895, #463), the absence of preview
and metadata in it (#465, #1085), the inability to skip a deleted file
(#651), the preview appearing after the window is closed (#1910), the broken search
for neighbors (#1021) and — the main one — the inability to SAVE the project after
relinking (#982). None of the eight is about how to FIND the file.
The difference is not explained by the presence of the window — Kdenlive has such a window too
(kde-invent #1437 names it: "Missing Clips or Files"
dialog) — and the three tasks out of 6346 that the same filter gives are almost all to the point
(#423960: "can search but didn't replace missing file path"). It reads like this: as soon as relinking becomes a GATE
on opening, it grows its own class of complaints on top of the original task.
We do not need this gate at all: the project opens with placeholders
(R.33 item 10), and relinking for us is an operation AFTER opening.

**The third pair — about universality.** The same filter gives exactly one task each
in flowblade (0.1 %) and shotcut (0.1 %). Three of the four are one and the same
request "pointed at one — the rest were found", submitted independently in
three projects and closed in all three as done (olive #610,
flowblade #929, shotcut #991); the fourth, kde-bugzilla #447827 ("bungs
all clips into the same directory"), came from the other side — it is the same
question of "what counts as one folder", asked not by relinking but by collection.
The fifth case, openshot #1829, the filter does not catch (the title "Media relinking
in file properties"), but it formulates the request verbatim. That is, the mechanism
is universal, cheap and satisfies everyone — and it is the very one that confuses same-named
files.

A mandatory review of `--rejected`: in this domain there are almost no refusals by maintainers —
the closest is kde-bugzilla #392963 "a "smart find" option for
finding missing clips", closed as INVALID in 2018. The function is not rejected,
it simply is not started: Pitivi #2050 (the archive does not open) has been open since
2017, kde-bugzilla #495091 (an absolute path in the project breaks moving a
directory) — since 2024.

## How it fits Kinotrain

### What already exists

Half of the task is solved by the FORMAT, and was solved earlier than the task was posed (R.24 item 7):

- `src/libs/kt-otio/src/url.rs` — `media_target_url()` when writing makes the media path relative to the project directory (POSIX slashes),
  `resolve_media_path()` when reading assembles it back; all three kinds are read (relative, absolute,
  `file://` with percent-encoding), `synth:<pattern>` passes through verbatim. The most frequent cause of relinking in
  all five core editors — "the whole folder moved together with the project" — does not happen with us.
- Because of this same thing, our `.otio` does not reproduce the Pitivi #2050 defect: the paths
  in the file and the layout on disk are consistent by construction, not by convention.
- `src/libs/kt-otio/src/convert.rs:50` — `ClipExtra::media_refs` +
  `active_key`: a clip in the file holds ALL `media_references`, and we
  replace only the active one. A place for "the old reference next to the new one" already
  exists, and the passthrough of foreign references is held by t144–t147.
- `src/libs/kt-model/src/project.rs:1777` — `MediaRef { id, path }`;
  a clip's reference to material is `ClipEvent::media: MediaId`, not a path
  (`project.rs:1794`). Editing a path in principle must not touch a single clip — this is exactly the design that relinking needs.
- The machinery for substituting a source on the fly is written and tested:
  `Player::replace_media` (`src/libs/kt-engine/src/player.rs:463`) stops the old
  `MediaWorker`, calls `cache.invalidate_source()`, raises a new one and bumps `project_version` so that prefetch wakes up;
  `KtApp::poll_returned_sources` (`src/apps/kt-app/src/app.rs:2131`) reopens the file in the background and calls it. Tests t211
  (`kt-engine/tests/player_dynamic.rs:204` and
  `apps/kt-app/src/layout_tests.rs:3795`).
- The placeholder in place of an unavailable file and the four levels of messages — F-HLP-012;
  from it exactly one thing is needed here: editing does NOT get destroyed when media is missing, therefore
  relinking for us is a calm operation after opening, not a condition of opening (t208–t210).
- The UI half of the relinking dialog is already written for another task:
  `crate::widgets::media_picker` / `media_picker_raw`
  (`src/apps/kt-app/src/widgets.rs:612` and `:657`) — a popup with search over
  the project's media list (R.35, test 218).
- **The contents of every media file are already hashed.**
  `MediaIndexDb::ensure_media` (`src/libs/kt-media/src/index.rs`) on the first addition of any file computes xxh3-128 over the whole content
  and puts it into the `xxhash` column next to `size` and `mtime_ns`. The material for
  relinking BY CONTENT — what none of the five core editors has — is already computed by us and lies on disk.

### What is missing

1. **There is no command for editing a path.** `kt_model::Command`
   (`src/libs/kt-model/src/model.rs:99` and `:213`) knows `AddMedia` and
   `RemoveMedia` and nothing between them. This confirms the finding of F-HLP-012,
   and it goes deeper than "forgot to add an enum variant".
2. **There is no workaround either — and this is by construction, not an oversight.**
   `RemoveMedia` is guarded by `CommandError::MediaInUse`
   (`model.rs:616`): media that at least one clip OR a morpher substitution points to cannot be removed. And a new `AddMedia` issues a new
   `MediaId`, so one would have to rewrite `ClipEvent::media` on all
   clips and all `MorphPatch` substitutions. A pair of commands that gives the effect
   of relinking does not exist in the model — today a file is repaired only by
   returning it to its former place.
3. **There is nothing to build the "same folder" hypothesis from.** The relative structure is not
   STORED with us; it is derived lexically at every write
   (`url::relativize`), and after loading the model holds absolute paths
   (`convert.rs:1362`). There is no volume name, no "root of the material", no memory of
   the fact that these forty files lay in one folder in the project.
4. **There is no collection at all.** The "File" menu
   (`src/apps/kt-app/src/toolbar.rs:246`) — New, Open, Import .ktproj,
   Save, Save As, Render, Quit. Neither "collect" nor "copy media with the project".
5. **Nothing in headless mode either.** `src/apps/kt-cli/src/main.rs:127` —
   `new`, `convert`, `demo`, `roundtrip`, `render`, `export`. Moreover,
   `roundtrip <in> <out>` takes `base` from the INPUT file in both
   operations, so it does not work as a way to "move a project" either.
6. **The bin card offers nothing.** The context menu
   (`src/apps/kt-app/src/bin_panel.rs:718`) — "place", grouping, locking, delete. The offline card gets a warning icon
   (F-HLP-012) and not a single action — a direct violation of rule 16.

### What gets in the way

**The index DB is keyed by path, and the content hash next to it is not used.**
`ensure_media` looks up the row `SELECT … FROM media WHERE path=?1`, and `path`
there is the RESOLVED ABSOLUTE path (item 150: a raw relative one gave the file
a second `media_id`). The hash is applied only to check "is this the same file at
the same path". There are two consequences, and both touch exactly this note:

- a byte-for-byte identical file after moving gets a new `media_id` and a full rebuild of
  `packet_index`, `audio_peaks`, `thumbnails`, `poster`, although the answer "it's him" lies in the neighboring column;
- `docs/media-index.md` describes `path` as "as in the project
  (relative)" — the documentation diverged from the code already in R.24 and promises
  exactly the property that is absent: there it also says "lies next to the project file → moves together with it", but with absolute paths inside, moving the directory zeroes the whole DB.

ADDRESS: src/libs/kt-media/src/index.rs — possible, and nothing catches it: `ensure_media` finds the media record only by the string of the absolute path, even though the xxh3-128 of the whole file lies in the same table row and is not a key — a byte-for-byte moved file loses its packet index, peak pyramid and thumbnails; and there is no way to fix the path, because `RemoveMedia` is closed by the guard `CommandError::MediaInUse` (`src/libs/kt-model/src/model.rs:616`), and no `kt_model::Command` changes `MediaRef::path`.

Other constraints:

- **Trimming for us today means re-encoding.** `Muxer`
  (`src/libs/kt-ffmpeg/src/muxer_impl.rs`) accepts only RGBA frames
  (`write_video`) and f32 samples (`write_audio`); there is no path for "move the packets
  without decoding". So the level "collect with trimming" is held up not by the UI,
  but by F-EXP-003, and without it would reproduce exactly the complaint that
  Vegas and Premiere get. Moreover, it would touch invariant 9 (the export is in-process) and 11 (determinism): the result of collection must be
  reproducible.
- **A relative path is not always built.** `url::relativize` returns
  `None` when the project directory and the material share only the root, and
  then an absolute `file://` is written. Material on a separate mount point (a memory card, a network volume) goes into the project as an absolute — and "the whole folder moved" stops being free for it. This is our variant of kde-bugzilla #495091.
- **The offline watchdog takes the list of paths ONCE** when the project is opened
  (`src/apps/kt-app/src/util.rs:183`). Any path edit must rebuild it, otherwise "repaired, but not released" — the same mine that F-HLP-012 named.
- **Copying gigabytes is a long background operation.** Rule 12
  of the checklist (background work does not get in the way of a gesture and of playback) and F-RES-012 (cancellation of long operations) apply in full;
  it is forbidden to create one's own progress strip (rule 13).
- **The cost of text.** Each line is five languages and a limit of 160 characters
  (`src/apps/kt-app/build.rs:42`), glyphs t100, subsets, t125/t128/t131;
  details — F-HLP-033.

### Which invariants it touches

Invariant 5 (exactly one `MediaWorker` owns the decoder) — the substitution goes only
through `Player::replace_media`; invariant 4 (the cache is the only truth about
coverage) — after a path edit the warmth bars must come from
`BurstCache::coverage()`, not from a separate bookkeeping; invariant 8
(scroll/zoom does not generate decode requests) — after relinking the index rows go stale, and until they are rebuilt in place the waveforms and thumbnails must
show placeholders, not decode in the draw path. The level "collect with trimming"
additionally touches 9 (the export is fully in-process) and 11
(determinism), which is why it is taken out to F-EXP-003.

The rules of the checklist in `src/apps/kt-app/CLAUDE.md`: 12 (background work does not
get in the way), 16 (impossibility is an ERROR: today the offline card offers not a single action), 19 (the dialog must say where to revert the decision —
critical for irreversible collection), 20 (nothing grows by itself: files
are copied by an explicit command, not "along with" the save).

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round, but in three separable levels: (1) point one media to a path — S; (2) the batch hypothesis "the same folder" + relinking by content from `xxhash` — M; (3) collection by copying WHOLE files + rewriting paths — M. Level (3-bis), collection with trimming to what is used, is not included in this size |
| Where the main work is | kt-model (the path-edit command with undo, a guard "path is not empty"), kt-app (a dialog on the ready `media_picker`, the item "File / Collect project…", an action in the bin card's context menu, rebuilding the watchdog), kt-media (lookup of the index row by `(size, xxhash)`, not only by path), kt-cli (`relink` and `collect` — the cheapest first form of all this), kt-otio (nothing: the paths are already relative) |
| Project format | does not touch it. The path already lies in `target_url`, the version `kt.v` does not grow; the old reference, if desired, goes in as a second key into the existing `media_references` map |
| Tests that will have to be written | a round-trip of the model on a path edit (undo returns the old one); "pointed at one file — the neighbors from the same folder were picked up" on the harness; "two same-named files from different folders do not get confused" — that very kde-bugzilla #456346 as a regression test; "the index row is reused by `(size, xxhash)` after the file moved"; "collection puts `synth:` verbatim and does not try to copy it"; "after collection the project opens from the NEW folder without a single absolute path"; kt-cli relink/collect |
| Depends on | F-EXP-003 (only the level "with trimming"); overlaps with F-MED-002 (proxies — the same source substitution) and F-HLP-012 (the message must name this command) |
| Breaks compatibility | no |

## Verdict

**We take levels 1–3, collection with trimming — rejected until F-EXP-003.** The reason for the refusal is not the size: without passthrough of packets, "trim" for us means passing the material through RGBA and re-encoding, that is, handing the user a generation loss disguised as saving space — exactly the complaint that
Vegas and Premiere have been collecting for twenty years. The right form is first
F-EXP-003, then trimming as its application.

The reconnaissance shifted the priority within the task. It was expected that the expensive part would be
copying; it turned out that the expensive part is **rewriting the paths**, and
exactly that is absent with us entirely: `MediaRef::path` is immutable, and
`RemoveMedia` is closed by the `MediaInUse` guard, so no combination of existing commands
gives the effect of relinking. Everything else already stands:
relative paths in the format (R.24 item 7) remove the most frequent foreign
scenario altogether, `Player::replace_media` can change the source on the fly
(t211), `media_picker` is a ready file choice, and the xxh3-128 of every media is already
computed and lies in `ktindex.db`. The latter is the only possibility in this note to overtake
the five, rather than catch up with them: relinking **by content** closes the name duplicates from different cards, which none
of the five solves, and along the way stops throwing away the index of a moved file.

Separately, a discrepancy found along the way and not related to the feature itself is recorded: `docs/media-index.md` describes the `path` column as
relative, while the code since R.24 (item 150) writes an absolute path there; because of
this the promise of the same document "the DB lies next to the project and moves
together with it" is not fulfilled.

## Sources

The five core editors — from vendor manuals read locally
(search over the extracted text); not one was run live. The open
editors — from the tracker exports in `_trackers.md`. The access date for
all — 2026-09-08.

**Core (manuals, read locally).**

- DaVinci Resolve 21 Reference Manual, Ch. 53 "Media Management", pp.
  1168–1175: the scope of the operation, `Copy`/`Transcode`, `Used media and trim
  keeping x frame handles`, `Consolidate multiple edit segments into one
  media file`, `Preserve folder hierarchy after x folder levels`,
  `Relink to new files`, "File Naming When You Consolidate Media"
  (`_S000`/`_0`), the compatibility of trimming with temporal codecs —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
- Final Cut Pro User Guide for Mac 12.3, "Consolidate projects and
  libraries", p. 857: the rule "outward — move, inward — copy", the scopes, media types, a separate consolidation of Motion content —
  https://support.apple.com/guide/final-cut-pro/consolidate-projects-and-libraries-ver9c7660349/mac
- Adobe Premiere Pro, "Project Manager" (section Project setup): trimmed
  project versus Collect Files, `Exclude Unused Clips`, `Include
  Handles`, `Make Offline`, `Rename Media Files To Match Clip Names`, and
  all three warnings — about Long GOP, multicam and "does not carry
  all effects". The local PDF of the Premiere Pro help was read; `[NOT
  VERIFIED]` for 26.x: `helpx.adobe.com` from this environment still does not open
  (a caveat of the first wave, `_editors.md`), so that the mechanism has not changed is confirmed only by the 2023 forum thread below
- After Effects, "Collect files in one location" (section Rendering and
  exporting): the scopes `All`/`For All Comps`/`For Selected Comps`/`For
  Queued Comps`/`None`, `Reduce Project`, `Obey Proxy Settings`,
  `Generate Report Only`, the contents of the report ("files, effects, and fonts").
  The local PDF of the After Effects help was read; `[NOT VERIFIED]` for 26.x
  for the same reason
- VEGAS Pro, "Copy media with project" / "Create trimmed copies of source
  media" with the field `Extra head and tail`, the list of trimmable formats and
  the phrase that the project is edited and saved. The local PDF of the
  VEGAS Pro manual was read (the 13/17 line, the project saving section);
  `[NOT VERIFIED]` for 2026.0 — the section was not found in the online help of Boris FX/MAGIX
  (the same failure recorded in F-HLP-012)

**Trackers** (the numbers are reproduced with `tracker-rank.py --query` with the filters
from the `SIGNAL:` lines; dumps — `~/.cache/kinotrain/trackers/`, they do not go into the repository):

- Kdenlive, Archive Project: https://bugs.kde.org/show_bug.cgi?id=447827
  (flattening), 456346 (same-named files get lost), 377999 and 427162
  (wrong paths in the collected project), 418301, 420494, 474792 (crashes), 501914
  (fonts), 420623 (the `Counter` generator), 492435 (built-in luma),
  383578, 505540, 492763 (scope of the operation); kde-invent 535, 561, 999
- Kdenlive, moving the project directory:
  https://bugs.kde.org/show_bug.cgi?id=495091 (open since 2024);
  "smart find" rejected as INVALID: 392963
- Pitivi: https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2050
  (archive with absolute paths), 982 (after relinking the project cannot be
  saved), 1021 (broken search for neighbors in the same folder), 1930
  (a deleted proxy is "missing" forever), 465, 463, 651, 895, 1085,
  1910 (complaints about the modal window)
- Flowblade, Media Relinker:
  https://github.com/jliljebl/flowblade/issues/929 (313 files → 4), 226
  (search also in the current directory), 958 (a file deleted forever — they ask for
  a placeholder), 907, 1175 (batch "ignore", open)
- Olive: https://github.com/olive-editor/olive/issues/610 (batch
  relinking), 680, 94
- Shotcut: https://github.com/mltframework/shotcut/issues/991 (point to a
  FOLDER, not a file)
- OpenShot: https://github.com/OpenShot/openshot-qt/issues/1829 (the most
  weighty task of the domain: 29 comments; the author fixes the loop "where is the file?" and
  at the same time asks for relinking to the RE-ENCODED version — that is, to
  the proxy), 1853

**Forums (quotes).**

- https://creativecow.net/forums/thread/copy-media-with-project-does-not-trim-the-source/
  (Vegas, 2013: why trimming is silent on inter-frame codecs)
- https://community.adobe.com/t5/premiere-pro-discussions/project-manager-not-reducing-clips-and-adding-handles/td-p/14322787
  (Premiere, 2023: the same complaint ten years later, with an explanation about i-frames from a Community Expert)

**What was searched for and NOT found** (so that the next wave does not search again):
relinking by a hash or content size — not a word in any of the five core manuals, only the name and path are compared; complaints about the loss of handles during collection — a filter over nine trackers
(`handles?\b.{0,25}(trim|export|media|clip)|(only|just).{0,15}used.{0,15}(part|portion|media|footage|clip)|trim\w*.{0,20}(unused|to.{0,10}used)|extract.{0,20}used`)
gives 9 tasks, of which NOT ONE is about collection: trimming during collection is a phenomenon of
commercial editors, and it is not measured by counting over open trackers,
so the section "The trick behind it" relies here on manuals and forums, not on
numbers. The `openshot` export was still in progress at the time of writing (4516 records out of
~4907 declared in the registry), so openshot is quoted, but is not taken into the percentage
comparisons of the `SIGNAL:` lines; `blender-vse` and
`ffmpeg-trac` were empty as of this date.
