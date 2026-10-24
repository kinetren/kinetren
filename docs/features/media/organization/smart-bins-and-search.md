---
id: F-MED-001
title: Smart folders, metadata and media tagging
status: candidate
priority: B
cost: L
category: media/organization
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-media, kt-ffmpeg, kt-app]
invariants: [4, 5, 8]
ui_rules: [13, 14, 17, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-002]
checked: 2026-09-07
---

# F-MED-001 — Smart folders, metadata and media tagging

## Essence

Two hours of footage is 300–400 files with names like `C0012.MP4`, and editing
starts long before the first splice: you have to find "that take
where he laughs" in three seconds. There are five layers of organization, and they lie on top of each other:
manual folders → smart folders (a saved QUERY instead of a list) → metadata
(from the camera and your own) → ratings, flags and color labels → and the last one,
which was carried through to the end only by FCP — marking PARTS of a clip, when
a keyword, "favorite" and "reject" are attached not to a file but to a range within it.

## Who has it and how

| Editor | Version | Name | How it works |
|---|---|---|---|
| Final Cut Pro | 12.3 | Keywords / Keyword Collections / Smart Collections / Favorite–Reject | the most elaborated model, and the only one where marking ranges is carried through to the end. Apple's help: "apply one or more keywords to a range within a clip, a whole clip, or a group of clips in an event"; a marked piece is flagged directly in the browser — "the clip or range is marked with a blue line in the browser", favorites with a green line, rejects with a red one. A Keyword Collection "contains pointers (aliases) to clips tagged with a specific keyword", and in it exactly the PIECE is visible. A Smart Collection is a saved Filter window: "Anytime a new item that matches the Smart Collection's search criteria is brought into the event or library, the new item is automatically added"; "Clips and projects that appear in Smart Collections are not duplicates". The criteria are listed by name: "clip or project name, rating, media type, used media, excessive shake, keywords, the presence of people, format information, date, roles, and item type". Rating is binary (F — favorite, Delete — reject, U — clear); the browser filter is a fixed set: All Clips / Hide Rejected / No Ratings or Keywords / Favorites / Rejected / Unused |
| DaVinci Resolve | 21 | Bins / Smart Bins / Power Bins / Metadata Editor | "Bins are folders that you create in the media pool to organize footage" — by hand; "Smart Bins use metadata to automatically organize your clips and are always kept up to date as new footage is added to your project" — a query, not a list; "Power Bins are visible across all projects on your system" — a level ABOVE the project. First-class metadata: "over 200 different metadata parameters for every clip", "both file and frame level metadata", and "Metadata tags can even be used to change the display names for clips using scene, shot and take information". Range marking exists halfway: a marker with a duration accepts keywords, but a smart bin returns the whole clip — see "The trick behind it", item 2 |
| VEGAS Pro | 2026.0 | Project Media: bins, smart bins, media tags, Search Media Bins, Use Count column | bins exist and they are virtual: "Bins are folders within projects that you can use to organize your media files", "They do not affect the way media is saved on your computer"; removing from a bin does not touch the project — "the file is removed from the bin, but remains part of your project". Some bins are created automatically: "Bins are automatically created for each media tag" and for each media type. There is a Search Media Bins dialog ("search the selected bin and all subbins"), and its result is saved as a smart bin: "Smart bins are saved search results that are automatically updated when new media items match the search results". There are no ratings, flags or color labels on the material; there is no range marking inside a clip; "Use Count" in Details mode shows how many times the material was taken onto the timeline, and in thumbnail mode this is not visible |
| Adobe Premiere | 26.x | Bins / Search Bins / XMP metadata in columns | folder-bins plus a **Search Bin** — a bin whose content is a saved query. The limitations are known from the Adobe forum and confirmed by an Adobe employee: bins are not search results — "Premiere Pro's search bins are designed to filter and display media assets based on metadata and labels, but they don't inherently support the inclusion of bins as search results" (Kevin J. Monahan Jr., Adobe, 08.10.2024); at most two conditions — "Two total search filter terms connected by AND and OR"; user-added XMP properties are not searched — "User-added custom XMP properties are nowhere to be had"; the Description field is not searched at all (the request has been open since 2016, see the quotes). There is no range marking inside a master clip: its role is played by subclips and markers with comments |
| After Effects | 26.x | Project panel: folders, search field, color labels | almost nothing: manual folders, search by name in the panel header, color labels on items. There are no smart folders, no ratings, no range marking — it organizes material not for selection but for assembling a composition. [NOT VERIFIED against the primary source: `helpx.adobe.com` does not open from this environment; the row is written from secondary mentions and was not checked live] |

## The trick behind it

**1. A smart bin is not a bin but a SAVED QUERY, and its whole essence sits in one field the user never sees.** The plus is obvious: a new file that matches the condition lands in it by itself, and it cannot "drift apart" from the material. The minuses are three, and all three surface only in practice. (a) You CANNOT drag a clip into it by hand: dragging does not change the condition, and membership is determined only by the condition, so a single exception clip ("this one take is needed after all") costs a rewrite of the rule, which affects the others. (b) Losing the condition means losing the bin: VEGAS's version history has the line "Fixed a bug that prevented smart bins from preserving their Condition setting". The bin survived saving, but its condition did not, and an empty box with a name remained. (c) The nastiest one: the condition compares STRINGS, not values, and this becomes noticeable with two-digit numbers. In Resolve, automatic smart bins by scene have for years put clips of scene 16 into both the bin "16" and the bin "6" (see the quote), because "6" is a substring of "16". On a shoot of nine scenes everything works, on fifteen it falls apart, and the editor discovers this not at setup time but two weeks later. A manual bin cannot break in this way at all.

**2. Marking a range is cheap, but SHOWING a range as a separate object is expensive, and Resolve got stuck on this.** In FCP, a marked stretch becomes an independent element of the browser: "when we click **Yellow
star** in the Library List, only that *portion* of the NASA Solar Flare
clip that was marked when we created the Yellow star keyword is displayed" (Larry Jordan). Mark fifteen stretches inside a one-hour interview and you get fifteen elements that copy nothing and do not break the link to the source. Resolve got exactly halfway: a marker with a duration accepts keywords, but the result is not right: "The filtered clips in the Smart Bin show the entire clip and not just what is between the IN and OUT of the Marker Duration" (Scott Simmons, ProVideoCoalition, 07.12.2018), and in eight years this has not changed: in 2024 editors write that automatic keyword bins "doesn't work with duration marker keywords". The conclusion for us matters more than the fact itself: the value lies not in storing a range, but in the range becoming a FIRST-CLASS element of the list, with its own name, its own poster and its own length. This is a decision at the level of the model, not the interface.

**3. Nobody searches by file name, because there is no file name.** The practical answer of the industry is not "search better" but "make the name meaningful": in Resolve, "Metadata tags can even be used to change the
display names for clips using scene, shot and take information. That means
you'll never have to rename media files again!" That is, the display name is GENERATED from the camera metadata. The second move is to search by what the person did not type: FCP's Filter criteria include "used media", "excessive shake" and "the presence of people", that is, analysis results. The third, the cheapest, is the "Use Count" column in VEGAS: the question "did I already take this?" is answered by a column, not by memory. And the mirror consequence: where there is no data, everything else breaks. Premiere's Search Bin has not searched the Description field since 2016, does not see colour labels and cannot handle user XMP properties, and takes at most two conditions, so a "smart bin" there regularly turns out empty, and people decide the feature does not work at all (the thread title is "Search Bins do absolutely nothing").

**4. Rejects are HIDDEN, not deleted, and this makes selection reversible.** "Rejecting
a clip does NOT delete it, it simply allows you not to display in the Event
Browser" (Larry Jordan), and Apple's help adds: "If you chose Hide
Rejected from the Filter pop-up menu at the top of the browser, clips
marked as rejected disappear from view". Hence a way of working that does not exist in the other four editors: editing by SUBTRACTION. You go through the material with two keys (F and Delete), then hide the rejects and look at what is left, and the reject is applied to a PIECE, not to a file. It is exactly the same logic as our automation keyframe behind a trim (F-AUT-002): the trim hides the keyframe rather than deleting it, and for the same reason this is safe.

## Pros

Consequences of the design, not ratings.

- A smart bin needs no discipline: the rule "everything that is 4K and longer than a minute" applies to a file imported tomorrow as well. A manual bin requires someone not to forget to put the file in.
- A query is more compact than a list: the condition "rating = Favorite" in the project file is one line, while the equivalent manual bin is a hundred references that must be fixed on renaming and moving.
- Marking by ranges removes a whole class of objects: a subclip, which copies the reference and lives its own life, is replaced by a mark on the source. Re-mark it and all collections rebuild; there is nothing to delete and recreate. Editors state this goal directly: "tag and organize media
without creating additional subclips which get messy with large
projects".
- Selection by subtraction is reversible: rejection hides rather than cuts, so "show everything" returns the material whole, and a decision from a week ago costs nothing to undo.
- Camera metadata gives what a person would never type by hand: timecode, lens, ISO, scene and take on 400 files. It is precisely their presence that decides whether the automation works: a generated name, a smart bin by scene and sync by timecode are one and the same dependency.
- A level above the project (Power Bins) removes the copying of logos and backgrounds between projects: one asset, many references.
- The virtuality of a bin removes the fear: in VEGAS, "the file is removed from the
bin, but remains part of your project", so you can sort material into folders without losing anything.

## Cons

The price of the same design.

- **A smart bin cannot be fixed by hand** — by construction it has no exceptions. In practice a manual bin is set up next to it, and the organisation splits in two.
- **One field holds the whole essence.** Losing the condition means losing the bin (a real VEGAS defect with `Condition setting`). A manual bin has no such single point of failure.
- **The condition compares strings.** Substring matching on scene numbers in Resolve has been alive since 2023 and lasts at least into 2024, and it breaks exactly when the project has grown, that is, at the most inconvenient moment.
- **Auto-update contradicts predictability.** The contents of a smart bin change without any gesture from the user — after an import, after editing metadata, after analysis. This is exactly what our UI checklist forbids with rule 20 ("nothing grows or gets cut off by itself"), and in other editors it regularly reads as "where did my clip go?". The extreme case: in Resolve, automatic keyword bins cannot be deleted: "Seems that any automatically generated keyword-based Smart Bins can't be removed from the project", and an empty folder disappears only after the project is reloaded.
- **A smart bin fits poorly with the rest of the interface.** In Resolve, the "find in media pool" command jumps to a static bin, so working inside a smart bin of 350 clips falls apart — a 2019 complaint repeated word for word in 2024. Smart bins do not see Power Bins ("Media is not "in" a project until you add to a regular bin"), and Power Bins themselves are not backed up, so people lose them together with the material sorted into them.
- **Range markings do not transfer.** Keyword ranges are a concept of the FCP library; moreover, they do not even transfer within FCP: "all of the information pertaining to the organizing and classifying of media is stored within the events and NOT the projects", so the logger marks, but the editor does not receive those markings. The most valuable layer of organisation is the least portable.
- **There are too many keywords.** FCP has exactly ten quick slots (⌃1…⌃9 plus Favorite); beyond that you reach for the mouse, and searching a list of eighty keywords turns into visual trawling. The advice of Larry Jordan himself — "Keep the total number of keywords small, so you don't overwhelm the
search box" — is in effect an admission of the system's limit.
- **Metadata is lost on the first re-encode** (a transcode to an "editing" codec or to proxies, usually not done by the editor, typically does not carry the camera tags over), and smart bins by scene/take silently empty out: automation that depends on data fails quietly. [NOT VERIFIED: no live complaints specifically about metadata loss on transcode could be found]
- **The media panel ages worse than the rest.** For VEGAS Project Media, the community says outright that it is "an old and fragile area of code", which is why the simplest indicator "already used" has not appeared for years; the typical workaround is to sort material into folders in the OS Explorer rather than in the editor's bins.

## What users say

A smart bin breaks on a substring, and has for years:

> However, clips from scene 16 are both in the smart bin for scene 16 and
> scene 6. And clips from scene 14 are both in the smart bin for scene 14
> and scene 4, etc.
> — DaniKamani (Daniel Kamangar), Blackmagic Design forum, 07.07.2023,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=184257

> The automatic smart bins are still not filtering correctly in v19.0.3 in
> Windows 11, apparently. I'm still getting scene 9 and scene 19 in the
> scene 9 bin.
> — Rakesh Malik, ibid., 20.10.2024

On Premiere and its Search Bins — an eight-year-old open defect, "search does not look in the Description field":

> I have children who have learned to walk, speak, run, read, play soccer,
> and do math in less time than it has taken Adobe to fix this critical bug.
> — Andy Adkins, Adobe forum, 09.01.2024,
> https://community.adobe.com/t5/premiere-pro-discussions/premiere-project-and-search-bins-not-searching-description-data/td-p/8737178

> I created a Search Bin filtering for 'Label - Magenta'…nothing. I
> literally saw a clip labled 'Magenta' direclty above the bin. What's going
> on?
> — channonp4602134, Adobe forum, 20.09.2025, thread "Search Bins do
> absolutely nothing",
> https://community.adobe.com/t5/premiere-pro-discussions/search-bins-do-absolutely-nothing/td-p/15513405

Why search by file name is useless — in the words of a VEGAS user (this is exactly the situation of "`C0012.MP4`"):

> While what @Jetdv posted can work, that means I would have to know the
> "name" of everyone clip. And since I don't rename them (they are the file
> names from the camera), that method doesn't work for me.
> — arpierre, Boris FX forum (VEGAS Pro), 10.05.2026,
> https://forum.borisfx.com/t/media-in-use-indicator-only-in-explorer-window-not-in-project-media/23315

Same thread — a request for flags and markers with a direct reference to Resolve as the benchmark, and an explanation of why the media panel is not being developed:

> I think Project Media can be more useful, besides of giving tag or
> keywords, for easy search during ongoing project editing, also, giving
> color tag or any other 'marker' or 'clue' (which Resolve call it as
> 'flag').
> — set_chung, ibid., 11.05.2026

> My memory of comments made was that whilst it would seem easy to do, it
> actually isn't. That's because it involves an old and fragile area of
> code.
> — ericlnz, ibid., 10.05.2026

A live defect of bin search in VEGAS 2026 — exactly our rule 14 ("a feature must work REPEATEDLY, not once"), broken at a neighbour's:

> Now however the search button is greyed out, so I can't run another
> search without closing and reopening VP.
> — wjauch, Boris FX forum, 20.04.2026,
> https://forum.borisfx.com/t/search-media-bins-only-allows-search-once/22660

On selection by subtraction, and on the ceiling that FCP's strong point has:

> Rejecting a clip does NOT delete it, it simply allows you not to display
> in the Event Browser.
> — Larry Jordan, 27.01.2013,
> https://larryjordan.com/articles/fcp-x-selecting-the-best-clips/

> I just keyworded several hours of footage with about 80 keywords and now
> I'm left searching for them visually in an alphabetical list.
> — Brandon VanBeek, comment, 22.03.2019,
> https://larryjordan.com/articles/fcp-x-find-media-using-keywords/

## How it fits Kinotrain

### What already exists

- **Media bin** — `src/apps/kt-app/src/bin_panel.rs`: 128×72 cards with a poster and scrubbing "by dragging" (48 logical scrub positions per file), a mini waveform instead of a black square for audio, two layout modes (grid and free positions, `bin_free`), auto-scroll at the edges (`bin_edge_scroll`), group dragging, and dragging onto the timeline with the margin `BIN_ESCAPE_SLACK`. Items 62, 99, 111, 118, 123.
- **Manual groups** — `BinGroup { name, members, locked }` in `src/libs/kt-model/src/project.rs`: a frame around the cards, the name in a gap in the top line, a lock. They live in `UiState`, that is, **outside undo**, and travel into the file via `KtTimelineMeta.bin_groups` (`src/libs/kt-otio/src/meta.rs`). This is exactly a "manual folder" — and nothing beyond it.
- **Project search (Ctrl+F)** — the pure `search_hits` in `src/apps/kt-app/src/app.rs`: it searches labels, the NAMES OF BIN GROUPS, media (file name and path), tracks and clips, ranking "exact match → start of string → substring"; live groups are passed as a parameter, otherwise the search would see only the saved ones (items 92, 99). Essential for this note: both `HitKind::Media` and `HitKind::Clip` take the string exactly from `Path::file_name(&m.path)` — **this is the search by `C0012.MP4`**, the very one the industry considers useless. There are no filters by type, no saving of a query, and no search by anything other than names.
- **Two markings of TIMELINE TIME.** `Marker { t, duration, name, color }` — a point and a range marker; `MarkedArea { name, start, duration, lanes, color }` — a named area "time × tracks", where `lanes` stores SEMANTIC keys, not row numbers (R.35 item 2). Both are in `src/libs/kt-model/src/project.rs`.
- **Sidecar index** — `src/libs/kt-media/src/index.rs`: the tables `media` (path, size, mtime, xxhash), `packet_index`, `audio_peaks`, `thumbnails`. The only place where anything "about the file" lives at all, and it is declared a CACHE: a broken DB is silently recreated.
- **Placeholders and source return** — `src/libs/kt-media/src/placeholder.rs`, the set `offline` in the bin (items 206–209): a missing source is visible, but this is about availability, not organisation.

### What is missing

- **There is no metadata at all, from the very bottom.** `MediaRef` is `{ id, path }` and nothing more (`src/libs/kt-model/src/project.rs`), and the command `AddMedia { id, path }` in `src/libs/kt-model/src/model.rs` is the same. Above it, `MediaInfo` (`src/libs/kt-media/src/lib.rs`) carries `duration/has_video/has_audio/fps/width/height` — and that is all; `kt_ffmpeg::Probe` (`src/libs/kt-ffmpeg/src/lib.rs`) has exactly the same six fields. **The function `av_dict_get` is not declared anywhere in the bindings** (`av_dict_set` is there — it is used to set encoder and muxer options): container and stream tags are physically never read by us. No timecode, no `creation_time`, no scene and take, no lens. A telling detail: in the schema of the sidecar DB the column `probe_json TEXT NOT NULL DEFAULT ''` is declared — and **it is not written or read by a single line of code**. Room for metadata was marked out and never occupied.
- **There are no ratings, flags or colour labels on material.** Colour exists on `Marker` and `MarkedArea` (for the round-trip of foreign OTIO), on media it does not.
- **There are no smart bins.** `BinGroup` is a list of `members`, and no place for a condition is provided in it.
- **There is no answer to the question "did I already take this?"** We have no analogue of the Use Count column, although the data lies at hand: `Project.clips` knows the `media` of every clip. This is the cheapest part of the whole note.
- **There is no marking INSIDE material, and this is a different domain, not an unfinished job.** Our marks and areas address TIMELINE TIME: `Marker.t` and `MarkedArea.start` are edit ticks, `MarkedArea.lanes` are tracks. FCP's keyword range addresses the MATERIAL TICK: it survives trimming, splitting, moving and reversing the event, because it knows nothing of the event at all, and one and the same marked piece is visible from all projects where that file is present. This is the same choice of domain that is already made and described for automation in F-AUT-002 (`AutoKey.t` = material tick) — only there it is made INSIDE the clip, whereas here it would have to be made for an entity that has no clip: the marking belongs to the FILE, not to the event. Our `Marker` cannot move into this domain — it lives in `Project.markers` and is measured by the timeline. The practical readiness criterion we take from Resolve, which got stuck on this: it is not enough to store a range — it must become a separate CARD in the bin, with its own name, poster and length.
- **Export of metadata outward is also empty.** When writing `.otio`, our own material receives `ExternalReference { available_range: None, metadata: {} }` (`src/libs/kt-otio/src/convert.rs`) — a foreign editor sees only the path. Foreign metadata, however, we do NOT lose: `meta_foreign` and `media_refs` are carried through as a whole passthrough, and the material's starting timecode is subtracted on reading and returned on writing (t180, `src/libs/kt-otio/tests/convert.rs`). That is, the source timecode passes through us, but it does not enter the model and is not visible to the user — ready data flies past.

### What gets in the way

- **Rule 13 "DO NOT WASTE SPACE IDLY" and the bin-width complaint repeated three times.** This is a direct constraint on any UI fantasies here. Verbatim from `docs/user-feedback.md`: "the media widget **FOR THE THIRD TIME AGAIN IS LIMITED BY WIDTH**" — the remark stands as item 4 in the section of R.12, while the summary tables of the same file and a code comment attribute it to item 4 of R.13; the topic itself is marked as repeated THREE times (R.9, R.12, R.13). In the same file, after R.13, about the cards: "the cards have become too large". Both decisions stand in the code with comments: in `src/apps/kt-app/src/app.rs` — "THE LIMIT IS SET BY THE COMPLAINT OF item 4 of R.13 — do not squeeze without a new request from the user" at `max_size((ui.max_rect().width() * 0.7).max(420.0))`, and in `src/apps/kt-app/src/bin_panel.rs` — "the card size is NOT inflated (complaint R.13): a wide column gives more cards in a row, not gigantic previews" at `card_w = ui.available_width().clamp(56.0,
PREVIEW_W)`. The practical conclusion: a tree of smart folders, metadata columns and a filter panel, each with its own strip, are NOT placed in this column. A rating and a flag must sit in space that is already occupied — a corner of the card, the colour of the frame, a line along the top of the preview (exactly what FCP uses to mark the marked pieces) — following the pattern of "the group name in a gap in the top line of the frame", which the checklist itself cites as the benchmark.
- **Rule 20 "nothing grows or gets cut off by itself" contradicts the very idea of a smart folder.** Formally there is no contradiction — the rule is about EDITING, and a smart folder edits nothing — but this has to be said aloud and shown in the interface: it must look different from a manual group, otherwise the first complaint will be "the card drove off by itself". Neighbours' users say exactly this.
- **Rule 17 "paired ways of doing one action behave the same".** A manual group and a smart folder look the same from outside ("a frame around the cards"), while their behaviour on dragging is opposite: a card is put into one, and not into the other. Either they look different, or it is a defect under the checklist.
- **Rule 14 "a feature must work repeatedly".** Three of five neighbours' implementations stumble precisely on this (VEGAS search greys out after the first query; Resolve's "find in media pool" throws you out of a smart bin; Resolve's automatic bins cannot be deleted). A test for a repeated query is mandatory from day one.
- **There is no one and no place to collect metadata.** Files are read by a separate thumb/probe worker (`src/apps/kt-app/src/thumbs.rs`; `duration_or_probe` places a request and wakes the worker), which also owns the sources and writes to the sidecar DB. Mass reading of tags on 400 files must go through this same path and in the background, otherwise invariant No. 8 (scroll/zoom does not generate decode requests) and checklist rule 12 (background work does not get in the way) are broken.
- **The project format needs a new level.** A manual group fitted into `KtTimelineMeta.bin_groups`; for a smart folder one must store a QUERY, and for marking material — an entity tied to the file, not to the timeline. Both have one place — `metadata.kt` (`src/libs/kt-otio/src/meta.rs`) — but a query that an old build reads as an empty group is exactly the case for which the bump `KT_META_VERSION` exists (today 4): silently losing the condition is worse than refusing (R.24 rule "ours, so that nobody breaks it").
- **We have nowhere to put a level above the project.** An analogue of Power Bins requires storage outside `.otio`; the sidecar DB is not fit for this role by its own rule — it is declared a cache and is silently recreated on damage (`src/libs/kt-media/src/index.rs`). An organisation that can be lost without warning is not an organisation; at Resolve this is exactly the complaint that Power Bins disappear and are not backed up.
- **The condition will have to be typed at once.** Resolve's defect with scene 16 in the scene 6 bin is not a detail of implementation, but a decision about whether the query compares strings or values. The specification of conditions must be a declaration of the same kind as `params_for!` for effects, not a set of string comparisons in the UI.

### Which invariants it touches

- **No. 8** (timeline scroll/zoom does not generate decode requests) — directly: filtering and showing metadata in the bin must not open files; everything is read once on import and lives in the index.
- **No. 5** (exactly one thread owns the decoder) — indirectly: tag collection goes through the same worker and the same priority queue, not through a new thread.
- **No. 4** (the cache is the single source of truth about coverage) — by analogy: metadata must have ONE place of truth; a separate bookkeeping of "what we know about the file" next to the sidecar DB is the same class of error. A subtlety: the DB is declared a CACHE, while user-entered tags and ratings are not a cache and cannot live in it.
- UI rules: **13** (space), **14** (repeatedly), **17** (paired ways), **20** (nothing grows by itself), plus **12** (background work does not get in the way).
- Engine invariants 1–3, 6, 7, 9–11 are not touched at all: this feature affects neither rendering, nor sound, nor export. A strong argument of "cheap in risk" with an expensive volume.

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round, if all five layers are taken. It splits into three independent parts: (a) metadata + rating/flag on the card + "already used" — S–M; (b) smart folders — M; (c) marking inside the material — a separate L, a new domain and a new kind of card |
| Where the main work is | `kt-ffmpeg` (wire up `av_dict_get`, extend `Probe`), `kt-media` (fill `probe_json`, which is already declared), `kt-model` (properties on `MediaRef`, a typed query condition), `kt-otio` (`metadata.kt` + version bump), `kt-app` (display on the card WITHOUT new strips — rule 13) |
| Project format | `KT_META_V5` is needed: a smart folder read by an old build as an empty group is a silent loss, whereas under the R.24 rule this is the standard `KtMetaTooNew`, not silence. Metadata and ratings by themselves do not bump the version if they travel with `serde(default)` |
| Tests to write | round-trip of `metadata.kt` with a smart folder and ratings; refusal of an old build on v5; evaluation of the query — a PURE function without UI (after the pattern of `search_hits` and `filter_entries`) with the mandatory case "scene 16 does not fall into the scene 6 bin"; repeated run of the query (rule 14); reading tags on the fixtures of `src/fixtures/` in `kt-media/tests/ffmpeg_conformance.rs`; preservation of `probe_json` on `Reused`/`Rebuilt` (`index_db`); a harness test of the bin in `layout_tests.rs` — no new strips appeared, column width unchanged |
| Depends on | F-AUT-002 — the already accepted distinction of domains "material versus timeline" comes from there; marking inside a clip must land in the same material domain |
| Breaks compatibility | yes, at the smart-folder layer (v5); the layers of metadata, ratings and "already used" — no |

## Verdict

We take it, but not whole and not at once. First — the cheap layer: reading tags (`av_dict_get` + an honest `probe_json`, for which room is already marked out), a rating/flag on the card and an "already used" indicator. It touches nothing in the engine and removes the main pain: `C0012.MP4` stops being the only thing we know about a file. Second — smart folders, necessarily with a DIFFERENT KIND from manual groups (otherwise a direct violation of rule 17), and with a typed condition from the very beginning: string comparison is a ready-made defect "scene 16 in the scene 6 bin", lived through by Resolve in front of everyone. Marking inside the material (keyword ranges) we move into a separate future note: this is not "one more mark" but a new entity in the material domain and a new kind of card in the bin, and it should be opened after media has properties at all. Separately recorded is the constraint: all of this must fit into the existing width of the bin — a topic the user has repeated three times, not subject to revision.

## Sources

- Final Cut Pro User Guide, "Add keywords":
  https://support.apple.com/guide/final-cut-pro/add-keywords-ver68416335/mac
  (accessed 2026-09-07)
- Final Cut Pro User Guide, "Create Smart Collections":
  https://support.apple.com/guide/final-cut-pro/create-smart-collections-ver2833eb5b/mac
  (2026-09-07)
- Final Cut Pro User Guide, "Rate clips":
  https://support.apple.com/guide/final-cut-pro/rate-clips-ver30ccd91f/mac
  (2026-09-07)
- Final Cut Pro User Guide, "Find clips and projects":
  https://support.apple.com/guide/final-cut-pro/find-clips-and-projects-ver65764b45/mac
  (2026-09-07)
- Blackmagic Design, DaVinci Resolve — Media page:
  https://www.blackmagicdesign.com/products/davinciresolve/media (2026-09-07)
- VEGAS Pro Help, "Using media bins" (line 22; in 2026 the section is the same):
  https://help.magix-hub.com/video/vegas/22/en/content/topics/4-media/media_bins.htm
  (2026-09-07)
- Scott Simmons, "DaVinci Resolve is a few steps away from range based
  keywording", ProVideoCoalition, 07.12.2018:
  https://www.provideocoalition.com/davinci-resolve-is-a-few-steps-away-from-range-based-keywording/
- Larry Jordan: "Use Keywords to Find the Clips You Need in FCP" (09.02.2020),
  "FCP X: Create Subclips Using Keywords" (18.01.2015), "FCP X: Selecting
  the Best Clips" (27.01.2013), "FCP X: Find Media Using Keywords" —
  https://larryjordan.com/
- Blackmagic Design forum: threads 184257 (substring defect of smart bins),
  134254 (non-deletable automatic bins), 217689 and p=1177703 (Power
  Bins), p=1026091 (keyword bins and range markers), 99513 ("find in
  media pool" throws you out of a smart bin) — links in the quotes section.
- Adobe forum (`community.adobe.com`): threads td-p/8737178 (Description is not
  searched, since 2016), td-p/15513405 ("Search Bins do absolutely nothing"),
  td-p/9229566 (custom XMP and the "two conditions" limit), m-p/14903370
  (an Adobe employee's reply about bins as search results).
- **The VEGAS forum has moved:** `vegascreativesoftware.info` and
  `vegascreativesoftware.com/us/forum` return a 301 to
  `https://forum.borisfx.com/` (Discourse, category Vegas Pro); the archive
  of old threads is unavailable, but the new forum's JSON search
  `/search.json?q=…` works. Threads used: 22660 (bin search works
  once), 23315 ("already used" indicator, flags, file names from the
  camera), 24284 ("Vegas Pro Release History" — fix for the smart bin condition).
- **Not verified against the primary source:** the After Effects line and part of
  the Premiere line. `helpx.adobe.com` does not open from this environment (see
  `docs/features/_editors.md`, section "What was unavailable"); the search page of
  `community.adobe.com` returns 403, although direct thread URLs open;
  search engines (DuckDuckGo, Bing, Brave, Ecosia, Mojeek, Yahoo, searx) answered
  with CAPTCHA, 403, 429 or junk. The Premiere wording is taken from the Adobe forum,
  including an Adobe employee's reply — this is a forum, not documentation, and is marked as such.
