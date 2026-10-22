---
id: F-RES-003
title: A project created by another version of the program
status: candidate
priority: B
cost: S
category: resilience/durability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-app, kt-model]
invariants: []
ui_rules: [16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-PRJ-002, F-RES-001]
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut, mlt, otio]
checked: 2026-09-08
---

# F-RES-003 — A project created by another version of the program

## Essence

The editor updated the program — and opens yesterday's project. Or the other way round: a colleague sent a file from a fresher build. There are exactly three outcomes: it opened as it was; it honestly refused with an explanation; **it opened, and parts of the edit are no longer in it**. The third outcome differs from the first in that it cannot be noticed: there was no error, the file is on the screen, the "unsaved" asterisk went out — and the next save writes the loss to the disk over the original. The first two cost irritation, the third costs the project and trust in the program forever.

## Who has it and how

The column "How it works" answers ONE question: what happens when the file and the program are from different versions. Autosave, copies alongside and the history panel are the subject of F-PRJ-002 and F-RES-001 and are not reopened here.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no name; a warning on saving | Forward only, and without exceptions: "VEGAS project files are always forwards compatible, but never backwards compatible". An old `.veg` can be opened by the new version, but **an alert is shown that saving will cut the file off from the earlier versions** — the only one of the five that warns BEFORE the loss. The reason for refusing backward writing is named directly: most of a `.veg` is a binary dump, not XML/JSON. The only historical exception is 11↔12 |
| Adobe Premiere | 26.x (2026) | no name | Forward only. In 26 the structure of the project changed for Object Mask, and a 26.x file with a substituted version number **crashes 25.x**, rather than refusing: the link sits in binary blobs. The official answer to the community is "nothing to announce" regarding backward compatibility; the standard workaround is one and it is lossy — export to FCP XML and import it into the old version |
| DaVinci Resolve | 21 | Database upgrade + Project Archive (`.drp`) | There is no project file — there is a database (Project Library), and a version change upgrades the DATABASE. The upgrade is irreversible and **warns beforehand**: having opened the project with the new version, you will no longer open it with the old one. The division of responsibility is honest: migration is a property of the storage, not of the document; the safety net is a `.drp` exported in advance |
| Final Cut Pro | 12.3 | Update Library / "created with a newer version" | A library package with a database. An old library is UPDATED on opening (on the switch to 10.3 — with conversion of compound/multicam/sync clips to the role format), there is no reverse path, and Apple explicitly advises keeping a copy and the old version of the application. In the other direction — a refusal by name: "created with a newer version of Final Cut Pro" |
| After Effects | 26.x (2026) | Save As → Save a Copy As 25.x / 24.x | **The only one of the five that can write backward.** The depth is finite and declared — two major versions; further only by a chain, each step done by the version that can still open the previous copy. That is, "both ways" here is not a property of the format but a service of the new build, and it lives exactly as long as the installed new build lives |

A line visible only when the table is assembled as a whole: **three warn, and one writes backward.** VEGAS, Resolve and FCP cannot return a file, but they speak of irreversibility at the moment it can still be avoided; Premiere cannot and does not say; After Effects can, but only two steps. The cheap half of the task (to warn) is covered by most, the expensive half (to return) by almost none, and this is exactly the proportion dictated by the design: backward writing requires knowing what exactly the new file has that the old one lacks, while a warning requires only the fact that it has more.

**Outside the core.** Two programs are needed for different reasons. Kdenlive is the only NLE whose document version and its migration are DESCRIBED in the documentation and visible in the public tracker, from which the count below is taken. The OpenTimelineIO library is the direct ORIGIN of both of our mechanisms (passive resilience to foreign schemas and versioning), and it is also the reference for our own format.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 (20.08.2026) | `kdenlive:docproperties.version` | The document version is an ordinary property inside the file (currently `1.1`). Opening a file of a lower version = **an upgrade of the document plus an automatic copy of the original** alongside: `myproject_backup.kdenlive`, which the user is told about. The refusal in the other direction, in words: "This project is unsupported because its document version is newer than what this Kdenlive release supports… Please consider upgrading your Kdenlive version" — that is, the message names the action. There is no backward writing: "Projects created with 23.04 forward are not backwards compatible" |
| OpenTimelineIO (library) | 0.18.x | schema versioning, upgrade/downgrade functions, version families | The only place in the domain where **downgrade is a declared API, not a service**: `target_schema_versions={'Clip':1}` when writing, `otioconvert --downgrade`, ready-made version maps by release, and user "families" (FAMILY) for studios. Plus a second mechanism living there too: an unknown schema does not break reading, but becomes an object that "remembers the original schema and contents blindly" (issue #326, 2018) — this is the ancestor of our passthrough |

## The trick behind it

**1. The version is determined BY CONTENT — and that is why it is not a ratchet.**
`kt_otio::project_to_timeline` does not store a number in the project and does not bump it on
save: each time it COMPUTES the version from what is actually in the model
(`src/libs/kt-otio/src/convert.rs:425-470`). A morpher with force or curves gives
v5; a morpher layer or a named region gives v4; FX automation, a chain master switch or a
non-crossfade transition gives v3; cameras give v2; otherwise v1. The consequence,
which none of the five has: **the version can go DOWN**. Verified by direct
experience on `src/fixtures/demo-3d.otio` (v3): we removed the cameras — the file was rewritten
still as v3 (the automation was in the way); we removed the automation too — `kt-cli roundtrip` wrote
`kt.v = 1`, and the file opens again in any old build. Compatibility for us is a property of the
EDIT, not a stamp on the file, and it is restored by removing the feature, not by a
"save for an old version" command. Hence the practical conclusion for planning: until a new
capability appears in a specific project, that project costs nothing — all files without 3D, automation
and morpher layers remain v1 and are byte-stable.

**2. Passthrough turns the format into a container: an old build not only
OPENS the new file — it can SAVE it, without eating anything.** This is not the same
thing and it costs more. Verified by experience: an unknown field was appended to our timeline
envelope (`future_timeline_thing`), an unknown field to the clip envelope (`future_clip_thing`) and
a whole foreign metadata namespace — after a round trip through
`kt-cli roundtrip` all three are in place, verbatim. The mechanism is `#[serde(flatten)] extra`
in `KtTimelineMeta`/`KtClipMeta` and `Retained.timeline_kt_extra`/`ClipExtra.kt_extra`,
plus `RawObject` for objects of unknown schemas. This is exactly what makes the
"forward only" policy almost free: while an addition does not change the meaning of what an old
reader ALREADY understands, the version does not need to be bumped, the file stays v1, and the editor
who sits on the previous build works with it without loss. Precisely this second
half is missing in Premiere and VEGAS: there a new field is a binary blob inside a
structure that the old version cannot walk past, so there any addition
automatically breaks compatibility.

**3. And the reverse side: "silently opened" and "silently lost" are distinguishable
ONLY by an explicit error — which is why `KtMetaBroken` costs more than it looks.** While
`kt_get` read our envelope through `serde_json::from_value(...).ok()`, one wrong
byte in any of its fields dropped the envelope ENTIRELY, and the file opened as a foreign
OTIO: without settings, tracks, morphers and layout — and without a single word (item 243,
found by a live scenario, closed by test 251). The difference between "we do not understand this" and
"we dropped this" is not visible in the UI or in the file — it is carried
solely by the error type. The general lesson: passthrough and silent loss look the same from
outside — "the file opened" — and only code that SPECIFICALLY distinguishes "the section is
absent" from "the section is broken" can tell them apart.

## Pros

- A version determined by content means compatibility breaks not with a release but with an
  EDIT: a user who did not touch 3D and automation does not pay for them having appeared in the program. All five
  core editors make everyone pay.
- The `KtMetaTooNew` refusal arises exactly where an old build would have rendered the wrong
  thing: a wipe instead of a crossfade, a whole layer instead of "a layer at 30 %", a curve in fractions
  instead of a curve in the material's ticks. This is not "we do not let in foreign files" but an
  enumerable list of reasons, each with its own constant `KT_META_V*` and a
  comment on WHAT exactly would have been spoiled.
- One version per FILE, not per object: nested envelopes (clip, effect, transition) carry no
  versions, so a "half-understood" state does not exist and the positions of children
  never become a guess.
- Passthrough gives what binary formats do not have at all: the file can be edited
  by someone else's tool (or by hand), and our data survives it. In this sense the format is not ours but
  shared — we rent a namespace in it.
- A spine version (Timeline/Stack/Track) newer than the supported one is a hard refusal before typed
  parsing. This is the rare case where refusal is cheaper than a guess:
  the order of children in OTIO is the edit itself.

## Cons

- Migration is one-way and irreversible INSIDE the file: `migrate_legacy_morpher`,
  `migrate_legacy_camera` and the conversion of curves from fractions to the material's ticks
  (`curves_from_progress`) only go up, and the next save writes
  the result over the original. No copy of "how it was" remains — unlike
  Kdenlive, which puts a `_backup` next to it.
- The `KtMetaTooNew` refusal is a dead end with no way out: there is neither "open read-only" nor
  "open, losing the new parts, with my consent", nor a link to a copy. A user with one file and an
  old build can do nothing.
- Version by content is a double-edged sword: it also means the moment "my project stopped
  opening on the second machine" comes not after an update but after ONE GESTURE — you added a camera, and the file became v2. Nobody warns about this.
- The file does not remember who wrote it. The envelope has `v`, but no build version
  and no date — the refusal message physically cannot name which version of the
  program to look for.
- The rule "bump the version only when the old reader would have spoiled the edit" is the judgment
  of the author of the change, not a check. An error in that direction ("it will do as it is") yields
  exactly the silent corruption that the rule exists to forbid, and it is caught only on the
  user's live files.

## What users say

SIGNAL: a project of a foreign version — 21 tasks in kde-bugzilla, `(older|newer|previous|earlier) version|after (the )?(update|upgrade)|downgrade|backward.?compat|(project|title)\w*\s+(file\s+)?(migrat|conver)`; share 0.3% versus 0.0% (1 task out of 2217) in kde-invent

The first number is small, and that is expected. The second is more interesting: **for ONE AND THE SAME
product, two trackers give a difference of an order of magnitude**. It is explained by dates: 8 of the 21
kde-bugzilla tasks were filed in 2015–2016 — the two releases when Kdenlive
rewrote the document format (one of them is even called "Project broken
after update to new kdenlive file format", 24.09.2015), while kde-invent opened
in 2019, when the rewrite had finished. The conclusion for which the count was taken:
**this class is not chronic, it is event-driven.** It does not "run in the background", it arrives
together with a format change and leaves together with it. For us this means that
the cost is paid not for having a migration, but for each bump of `KT_META_V*` separately — and it has to be planned for the bump, not for the release.

The second thing visible only from the count: **the share of real fixes is 9 of 18 closed
(50 %)**, and of the remaining ones four were closed as `WORKSFORME` and two as `WAITINGFORINFO`.
A third of the closed ones were written off as "not reproducible" — for a defect that requires a
second installed version and a file from it, this is the expected outcome, and it also
explains why the count is small.

The refusal that looks legitimate, and is also the most common: "we do not support opening in an old
version". The vendor's verbatim wording is from the Kdenlive help, and it is more honest than most,
because it names the action:

> This project is unsupported because its document version is newer than what
> this Kdenlive release supports… Please consider upgrading your Kdenlive
> version.
> — docs.kdenlive.org, Project File Details, accessed 08.09.2026,
> https://docs.kdenlive.org/en/project_and_asset_management/file_management/project_files.html

And here is how the same refusal looks when it was not phrased at all — the headings verbatim, all three about our class:

> Cannot open projects made with previous version
> — Kdenlive #406872, 25.04.2019, closed WORKSFORME,
> https://bugs.kde.org/show_bug.cgi?id=406872

> Segfault while trying to play clips in project made with previous version of
> Kdenlive
> — Kdenlive #1571, 09.11.2022, https://invent.kde.org/multimedia/kdenlive/-/work_items/1571

> Crash when loading a project with a newer version
> — Shotcut #624, 26.08.2018, https://github.com/mltframework/shotcut/issues/624

Now — about the third policy, for which this note was written. **Silent loss is almost invisible in the trackers, and not because there is little of it: there is nothing to see it with.**
The only place in all six exports where it is counted is a cluster of
five tasks in kde-invent, filed over two weeks in October 2022 by one
person who created a SEPARATE repository with projects of old versions
(`invent.kde.org/erjiang/kdenlive-backcompat`) and rendered them in two builds
in a row. The query `renders? (differently|different )|looks? different|different
number of frames` gives 5 tasks out of 2217 (0.2 %) in kde-invent and 0.0 % in all
six other trackers — and **all five are open to this day, the fourth year, not one
is closed**. Neighbouring tasks from the same two weeks (#1536–#1590) are closed
more than half. The wording:

> See that the final section of the video looks very different — the soft glow
> in 20.12.1c looks instead like a flat brightness across the entire frame.
> — Kdenlive #1539, 10.10.2022, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1539

> See that the render output of 20.12.1c has 300 frames and the render output
> of 22.08.1 has 301 frames.
> — Kdenlive #1540, 10.10.2022, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1540

This is the real subject of the note in someone else's execution: the file OPENED, there was no error,
no check fired — and the picture is different. One can notice such a thing
only by having the old version, the old project and the patience to render both. Two conclusions follow, which
are not visible in a single task. First: the count on this topic must not be read as "rare" — it must be read as "nobody has measured it".
Second: the only known way of measuring is **a corpus of archived files
rendered in regression**, and in this domain it has been invented exactly once, by an outsider.

And separately — from the tracker of the format library itself: OTIO has had schema upgrade/downgrade
as an API since 2016 (issue #10), but the task "leave a trace in the file
that it was downgraded" (#1392, "Upgrade/Downgrade breadcrumbs", 10.09.2022) has been open for
the fourth year. Even where a downgrade is implemented, the question "how do you later find out that
the file went through a downgrade" has not been solved by anyone.

## How it fits Kinotrain

### What already exists

- **Version by content, computed on every write.**
  `src/libs/kt-otio/src/convert.rs:425-470`: five constants `KT_META_V1..V5`
  (`src/libs/kt-otio/src/meta.rs`), each with a comment on WHAT exactly an old build
  would spoil. The version can also go down: checked by `kt-cli roundtrip` —
  a v3 file is written as v1 after the cameras and automation are removed.
- **An honest refusal in both directions.** `Error::KtMetaTooNew(v)` — the file is newer than our
  build; `Error::UnsupportedSchema` — the version of the SPINE (Timeline/Stack/Track) is
  newer than the supported one, checked by a preflight on the raw `Value` BEFORE
  typed parsing, because otherwise the positions of children would become a guess.
  `Error::KtMetaBroken` — the section is present but cannot be read (item 243, test 251).
- **Three-level passthrough, verified by experience.** Unknown fields of OUR envelope
  (`#[serde(flatten)] extra` in `KtTimelineMeta`/`KtClipMeta`, carried
  via `Retained.timeline_kt_extra` and `ClipExtra.kt_extra`), objects of unknown
  schemas (`RawObject`, on the timeline — a grey placeholder), foreign namespaces of
  `metadata` — everything survives a round trip verbatim.
- **Upgrade of old domains on read.** `migrate_legacy_morpher` for files
  younger than v4, `migrate_legacy_camera`, the conversion of curves from fractions of the event
  length to the material's ticks (`curves_from_progress`) for files younger than v3 — that is,
  "the new one reads the old" is implemented not as "read and forgot", but as a
  meaningful conversion of the domain (tests 189, 212, 222).
- **The only archive corpus in the tree.** `src/fixtures/demo.ktproj` is a real file written by
  the PREVIOUS PROGRAM (C++ v1), and test 148 pins the byte-for-byte result of its
  import. The method has already been invented and works — it is simply applied to one boundary out of six.

### What is missing

- **The same defect as item 243 lives one floor down and is not closed.** At the level of the CLIP the envelope
  is read by the same `kt_get` (`src/libs/kt-otio/src/convert.rs:1156`), that is,
  through `.ok()`. Verified by experience: I broke one type in `Clip.metadata.kt`
  (`"gain": "loud"`) — `kt-cli roundtrip` returned **exit code 0, zero warnings**, and in the
  written file the clip has a new `id` (110 → 1), and `link_group: 5` and
  `gain: -3.0` disappeared. Silently, over the original. A broken `id` at the same time loses the
  binding of morphers, link groups and `MorphTargets::Clips` targets. In one of two
  runs the new id coincided with an existing one, and the user would get the message
  `unsupported schema version: invalid timeline: duplicate clip id ClipId(1)` —
  that is, a wording about the schema version where our field is broken.
  Test 251 catches exactly this class, but only at the timeline level.
- **There is no warning about the version bump at all.** `project_to_timeline` does not
  accept `notices` and silently raises the file version: you add a camera — the file became
  v2 and stopped opening on a second machine with yesterday's build. Three editors out of
  five say so out loud in this situation; for us nobody says anything.
- **There is no downgrade migration in any form** — neither `save_project` with a target version
  nor an export. At the same time the format HAS such a mechanism at the level of schemas
  (`target_schema_versions`, `otioconvert --downgrade`), and it does not help: our data lives not in the schemas but in
  `metadata.kt`, which the OTIO downgrade functions do not touch. So there is nothing to inherit — we would have to write our own.
- **There is no copy before migration.** A file younger than our version is read, the domains are
  converted, and the next save (including the background autosave every
  30 s, `src/apps/kt-app/src/app.rs`) writes the result OVER it. Kdenlive in the
  same situation puts a `_backup` next to it; we have no copy next to it at all (F-RES-002).
- **The refusal message is not finished.** `load_project_file`
  (`src/apps/kt-app/src/app.rs:4957`) returns `e.to_string()`, and the user
  sees "English: "did not open"" — a Russian frame —
  a Russian frame, an English tail, neither its own version, nor the required one, nor what to do
  next. The catalogues `i18n/*.toml` know only the `open_failed` wrapper. The file does not carry its own
  version at all: the envelope has `v`, but no build and no date,
  so naming "which program to look for" is physically impossible today.
- **There is no corpus of files written by RELEASED builds.** Both golden fixtures are generated by our
  own writer (`t148_regen`, `t189_regen` in
  `src/libs/kt-otio/tests/convert.rs`), and test 212 "a file from before v3" synthesises
  it by patching JSON from a fresh write — so drift in the writer will drag the "old" file along with it, and the test will stay green. `src/fixtures/otio-samples` covers
  the boundary "a foreign tool", but not the boundary "our previous version".
- **The file version is visible nowhere.** In `src/apps/kt-app/src/` the envelope version is not shown in a single place;
  `kt_version::display()` is the version of the BUILD. The editor cannot answer the question "will this file open at a colleague's?".

### What gets in the way

- A bump warning is not a string but a decision about "where". To show it at the moment of
  the GESTURE (you added a camera) means pulling the user away from work; to show it on save means inserting a banner into Ctrl+S, and saving for us also runs
  through the background autosave, which under rule 12 has no right to show anything. Rule 19 adds a condition: the banner must say where this is switched back,
  and "back" here means "remove the feature from the project".
- A downgrade migration is not a function but a second semantics of the format: for every pair of versions
  one must decide WHAT to do with what cannot be translated (a morpher layer in a model without layers,
  a curve in the material's ticks for a reader expecting fractions). This is an enumerable but not small
  job, and it must be an explicit loss with confirmation, otherwise it turns into exactly the class against which the note is written.
- A corpus of archived files requires release discipline, not code: a file goes into the corpus
  by the build that wrote it, and it cannot be created after the fact. The later we start, the less of it there will be.

### Which invariants it touches

None of 1–12 of the root `CLAUDE.md`: reading and writing the project touch neither the audio path,
nor the FX state, nor the cache, nor render determinism. Three items of the checklist in
`src/apps/kt-app/CLAUDE.md` are touched, and only on the side of the message, not the engine: **16**
(an impossibility is shown as an ERROR, not as a soft line — today the refusal goes to the log as a raw English tail),
**19** (the banner about the version bump must name where the decision is reversed) and **20** ("nothing is switched off by itself" — a silent
reassignment of `ClipId` on a broken clip envelope violates it literally). The absence of engine invariants is a strong argument for the cost.

ADDRESS: src/libs/kt-otio/src/convert.rs:1156 — the class "silently opened and silently lost" is POSSIBLE for us and nothing catches it: the CLIP envelope is read by the old `kt_get` via `.ok()`, and `kt-cli roundtrip` over a file with one wrong type in `Clip.metadata.kt` returned code 0 without warnings, reassigning `id` 110 → 1 and losing `link_group` and `gain`; test 251 closes exactly this class, but only at the level of `Timeline.metadata.kt`.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round |
| Where the main work is | kt-otio (`kt_get` → checked variant at the level of the clip, the effect and the transition curve; the envelope version exposed from `ProjectBundle`), kt-app (the version-bump banner, the finished refusal message), fixtures (archive corpus) |
| Project format | not touched; the field "who wrote the file" is a separate decision, does not bump the version (goes into `extra` and travels by passthrough to old builds) |
| Tests that will have to be written | "a broken CLIP envelope is an error, not a new id" (continuation of 251 to the second level); "the version bump is visible to the user"; a regression on the archive corpus: a file written by a released build is read and yields the same model |
| Depends on | F-RES-002 (a copy next to it before an irreversible migration), F-PRJ-002 (project versions), F-RES-001 (autosave, which is what writes the loss) |
| Breaks compatibility | no |

## Verdict

We take it, and take it narrowly. Our case is strong: a version determined by content instead of a number in the
header gives compatibility for free — a file without new fields is read by any
build and stays byte-stable, and passthrough allows an old build not
merely to open a new file but to SAVE it, without eating anything. This is better than
all five core editors, and there is nothing to redo here.

One thing must be fixed, and for one reason: **the hole of item 243 is closed at the
timeline level and open at the clip level**, and this is verified not by reasoning but by a run —
exit code 0, zero warnings, lost `link_group` and `gain`, a new
`ClipId`. As long as `kt_get` somewhere reads our envelope via `.ok()`, we run on
the third policy — the very one whose analysis this note was written for.

The second step of the same stage — to speak out loud about the version bump (today the file
becomes v2 from one gesture and silently stops opening on a second machine) and to bring the refusal message to the state of "which version is needed and what to do".
We do not take the downgrade migration: the format has such a mechanism at the level of schemas and it does not help our
data, we would have to write our own, and the cost is a second semantics of the
format for every pair of versions. The right answer to "I want to open it in an old one" we already
have, and it is cheaper: remove the feature — and the file is v1 again.

A separate line, as the cheapest insurance against what we cannot
measure: a corpus of archived files written by RELEASED builds. The method is
already in the tree (`src/fixtures/demo.ktproj` + test 148), it costs one file
per release, and without it we are exactly where Kdenlive was before October 2022 —
the class exists and nobody has counted it.

## Sources

- `python3 src/scripts/tracker-rank.py --query '(older|newer|previous|earlier) version|after (the )?(update|upgrade)|downgrade|backward.?compat|(project|title)\w*\s+(file\s+)?(migrat|conver)'` — 21 / 6346 (0.3 %) kde-bugzilla, 1 / 2217 (0.0 %) kde-invent, 3 / 680 (0.4 %) otio, 3 / 1368 (0.2 %) shotcut, 1 / 601 (0.2 %) mlt, 1 / 2672 (0.0 %) pitivi (08.09.2026)
- `python3 src/scripts/tracker-rank.py --query 'renders? (differently|different )|looks? different|different number of frames'` — 5 / 2217 (0.2 %) kde-invent, 0.0 % in the other six; all five open (08.09.2026)
- Breakdown of the 21 kde-bugzilla tasks by year and outcome (8 of 21 in 2015–2016; closed 18, real fixes 9, `WORKSFORME` 4, `WAITINGFORINFO` 2) obtained by a direct pass over `~/.cache/kinotrain/trackers/kde-bugzilla.jsonl` — `--query` prints only the shares
- **The otio share (0.4 %) is read with a caveat:** of the three tasks, two are about the versioning MECHANISM itself (#1392, #1425), and the third (#905) is CI noise. This does not mean "the format has more pain" but "for the format, versioning is the subject"
- https://bugs.kde.org/show_bug.cgi?id=406872 · https://bugs.kde.org/show_bug.cgi?id=353125 (titles and statuses from the dump of 08.09.2026; the kde-bugzilla export contains no task bodies)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1571 · /1539 · /1540 (accessed 08.09.2026, from the dump with bodies)
- https://github.com/mltframework/shotcut/issues/624 (accessed 08.09.2026, from the dump)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/10 · /326 · /1392 (accessed 08.09.2026, from the dump)
- https://opentimelineio.readthedocs.io/en/latest/tutorials/versioning-schemas.html — downgrade, `target_schema_versions`, `otioconvert --downgrade`, version families (accessed 08.09.2026)
- https://docs.kdenlive.org/en/project_and_asset_management/file_management/project_files.html — `kdenlive:docproperties.version`, `myproject_backup.kdenlive`, refusal by document version (accessed 08.09.2026)
- VEGAS: `moviestudiozen.com` and the forum `vegascreativesoftware.info` — "always forwards compatible, but never backwards compatible", an alert when an old project is saved by a new version, the cause (a binary dump), the exception 11↔12. **From secondary sources and the vendor's forum, not checked live**; the page `moviestudiozen.com` did not open from this environment (proxy), the wording is from search results of 08.09.2026
- Premiere: `community.adobe.com`, thread "Has version "2026" killed the ability to downgrade a project to a previous version?" (26.01.2026) — the Community Manager's answer "We haven't intentionally prevented downgrading tools from working with Premiere v26"; the behaviour of 25.x on a 26.x file with a substituted version — from secondary sources, **not checked live**
- Resolve: the database upgrade is irreversible, a warning beforehand, "projects opened in 21.0.4 are unavailable in 20.3.2" — from secondary sources and the BMD forum (08.09.2026), **not checked live**
- Final Cut Pro: `support.apple.com` — the library upgrade on moving to 10.3 (conversion of compound/multicam/sync into the role-based format), a recommendation to keep a copy and the old version; the refusal "created with a newer version of Final Cut Pro" — from user discussions (08.09.2026), **not checked live**
- After Effects: `blog.adobe.com` and secondary sources — Save a Copy As two major versions back (in 26.x — 25.x and 24.x), then in a chain (08.09.2026), **not checked live**
- Our code was checked by grep and by a RUN: `src/libs/kt-otio/src/{lib,meta,convert}.rs`, `src/libs/kt-otio/tests/{format,convert}.rs`, `src/apps/kt-app/src/app.rs`. The experiments were run with `target/debug/kt-cli roundtrip` on copies of `src/fixtures/demo-3d.otio` in a temporary directory: (1) the version falls v3 → v1 after the cameras and automation are removed; (2) unknown fields of the timeline and clip envelopes and a foreign namespace survive a round trip; (3) one wrong type in `Clip.metadata.kt` gives exit code 0, zero warnings and the loss of `id`/`link_group`/`gain`; (4) `kt.v = 9` gives `project written by a newer Kinotrain (kt.v = 9)`. The working tree was not changed
- **Six trackers out of twelve were exported** (kde-bugzilla 6346, kde-invent 2217, pitivi 2672, shotcut 1368, mlt 601, otio 680; olive was still downloading at the time of writing, dumps dated 08.09.2026); openshot, flowblade, losslesscut, blender-vse and ffmpeg-trac were not exported — the numbers of this note do not refer to them
- The mechanisms of autosave, copies next to it and write atomicity were not re-examined: F-PRJ-002, F-RES-001, F-RES-002. Divergence of preview and output WITHIN one version — F-RES-007; here it is only about divergence BETWEEN versions
