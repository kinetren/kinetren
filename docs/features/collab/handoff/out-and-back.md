---
id: F-COL-009
title: Handed off for color — taken back
status: candidate
priority: C
cost: S
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: no
crates: [kt-otio, kt-model, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-COL-007, F-COL-008, F-MED-005]
trackers: [otio, olive]
services: [bm-cloud, resolve-project-server, frame-io]
checked: 2026-09-08
---

# F-COL-009 — Handed off for color — taken back

## Essence

The edit has gone to the colourist (or to titles, or to graphics), and a week later something
comes back from him. Not a project: the colourist has a different programme and a different
document. What arrives is either a set of finished files, or a list — "on this frame
such a grade". The question of the note: by which key is this glued to the edit,
which in a week could have changed. The giving-out is F-COL-007; here there is only the return
road and what is measured on it for coincidence.

## Who has it and how

The column "How it works" answers one question: what comes BACK from the receiving side
and by which key it is glued to the edit. What goes there and in what format — F-COL-007; the fate of a
foreign file inside us — F-COL-008.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no name | only FILES come back — finished graded shots or the master. There is no mechanism of "accepting a property" in any form: the event is re-linked to the new file by hand, and the key of the splice is a person |
| Adobe Premiere | 26.x | Lumetri → Import LUT (accepts `.cube` and CDL); the return of the edit — XML/AAF | the colour comes back as a TABLE PER CLIP: the file is chosen manually and hung on the event, there is no time and no timecode in it. Adobe had a live channel of "colour as data" and it died together with SpeedGrade (Direct Link); manual loading remained. `[NOT VERIFIED]` — from the Lumetri documentation and forum threads |
| DaVinci Resolve | 21 | ColorTrace, Remote Grades, Reconform from bins, Conform Lock | the only one whose return is a MECHANISM, not a gesture. ColorTrace transfers grades from one timeline to another by the SOURCE TIMECODE and the reel name (in automatic mode also by the clip name); a repeated conform re-seats the updated edit onto already graded shots |
| Final Cut Pro | 12.3 | Import XML; media by replacement | `.fcpxml` and files come back. The colour as DATA does not go through the library: a render comes back, not a decision, and it is glued by replacing the material |
| After Effects | 26.x | — | there is no return at all: AE has an importer and no exporter of the edit. The road is one-way by construction, and "accept back" is not defined here |

**Outside the core.** The five describe this feature with verbs ("send", "receive"), and
from their documentation one cannot understand WHAT travels back as a data structure.
OpenTimelineIO is the only place where the return channel has a schema, field names
and a public list of what is missing in it.

| Programme | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | adapters `ale`, `cmx_3600` (CDL in `metadata["cdl"]`) | the return exists as data: slope/offset/power and saturation lie in the clip's metadata in the fields `ASC_SOP` and `ASC_SAT`. And precisely here the asymmetry of direction is visible: the ALE adapter READS these values and does not write them back (#1050, open since 2021), and the CDL itself is "not unified among adapters" (#619) — that is, even where the return is described, it is described in one direction only |

## The trick behind it

**The exchange is one-way by DATA and two-way by TIME.** STRUCTURE goes there —
what follows what, where the splice is, how long it lasts; a PROPERTY comes back —
which grade lies on which frame of which material. These are different types, and therefore
the return channel has the right to be poorer than the direct one: a list of ten numbers per shot
(slope/offset/power × RGB plus saturation) needs neither tracks nor
nesting. Hence also the explanation why the reverse direction is unfinished everywhere: it is not needed
symmetric by anyone, and, for example, the ALE adapter of OTIO has been reading CDL for the fourth year and still does not write it.

**The key of the splice is not an identifier but the clock of the MATERIAL.** Two documents
made in different programmes have nothing to present to each other: they have no common id and cannot have one.
Therefore ColorTrace compares the source timecode and the reel name — a coordinate that lives not in the document but in the
shot material itself. This is a seam left over from film and tape, and it has held the
file-based industry up to now precisely because nothing better has appeared.
Direct proof: when there is no reel (and in the file world there is none), Resolve
offers to MAKE one from the path to the file — the setting "Assist using reel names from
the source clip file pathname". That is, the industry officially declares **the address of the
material to be a surrogate of the reel**, and the return channel in the end holds on the string of the path.

**And third: locking exists here, but it is not in the code.** While the colour is being done, the edit
is not locked by the programme — it is locked by the promise of picture lock. This is pessimistic
locking in its pure form: the unit of occupancy is the whole edit, the term is until the return, the mechanism is agreement. Zero lines of code, and it works exactly as reliably as the
Final Cut lock on the whole library. ColorTrace exists not in place of this lock but BECAUSE it is broken: it is needed on the day when
the edit was changed after all, and the cost of the breach is re-seating the grades by timecode with a
manual pick-up of those that did not match.

## Pros

- The industry's mechanism requires neither a server, nor a user account, nor shared wall-clock time: the key lives in the material, which is the same for both sides. This is the only known scheme of joint work that works entirely through a flash drive.
- The return channel is poor, and therefore verifiable: ten numbers per shot can be read through by eye, like EDL.
- A lock by agreement has no state and therefore does not break: there is nothing to hang, nothing to unlock, there is no "the lock was left by someone who was dismissed".
- The separation by data type gives a natural boundary of responsibility: the structure belongs to the editor, the property to the colourist, and they do not dispute one record.

## Cons

- **The key is approximate, and this is admitted aloud**: a match by timecode that is "not even exact" gives false positives on material where the timecode
  starts from zero for each file (and file-based material is exactly like this).
- **The surrogate of the reel from the path breaks exactly when the material has been moved** — that is, in the very scenario for which relative paths were devised.
- **The agreement is not observable**: the programme does not know that the edit is locked, and cannot say that it has been touched. This is learned a week later, when the re-seating of grades gives half of the mismatches.
- The return as a "property" works only for colour. Titles, graphics and sound come back as files, and there the key of the splice is again a person.
- Four of the five have no mechanism at all: the return is reduced to replacing the material by hand, and the cost of the error is visible only on viewing.

## What users say

SIGNAL: conform and return of grade — 13 tasks in otio, `\b(conform|reconform|re-conform|color ?trace|cdl|ale)\b`; share 1.9% against 0.1% in olive

The topic lives almost entirely outside the editors: 1.9% at otio against 0.1% at olive and
0.0% at pitivi; for kde-bugzilla, shotcut, openshot and flowblade the filter gives not a single match. The conclusion is the same as in F-COL-007, but with the opposite sign: conform —
is not a function of the editor, but a separate craft, and it lives in the exchange libraries. For
us this means that searching here for "how it is done in NLE" is useless: it is not done in NLE.

The key of the splice is named in plain text, together with the reason why it is approximate:

> ColorTrace was designed for film and tape workflows where a clip is identified by its reel and timecode. It has not (yet?) adapted to a file-based workflow where the main identifier for a clip is the file name.
> — David Hover, ProVideo Coalition, 15.10.2018,
> https://www.provideocoalition.com/solutions-to-resolve-part-2-accurate-shot-match-in-colortracetm/

The practice of return, described by an editor as a sequence of actions — and it is visible that "accept back" here means to COMPARE two timelines, not to open the document sent:

> you bring a new AAF into Resolve (of the new edit from Avid). Then you compare it to the original graded timeline with Color Trace.
> — Scott Clements, Creative COW, 15.01.2015,
> https://creativecow.net/forums/thread/how-do-timelines-get-updated-when-the-edit-changes/

When the return is set up, it ceases to be an event and becomes background — and this is the sign that the channel is poor and therefore reliable:

> if there are new XML:s coming your way every once in a while, it's lightning fast to just import the XML and boom, your grade is ready.
> — Timo Teravainen, Creative COW, 25.01.2015,
> https://creativecow.net/forums/thread/how-do-timelines-get-updated-when-the-edit-changes/

And the reverse direction, in the only place where it is described at all as data, has not been finished for the fourth year: `otio #1050` ("reads CDL values … however it does not
write them back out", reinecke, 24.08.2021) — the issue is open on the date of verification.

## How it fits Kinotrain

ADDRESS: src/libs/kt-otio/src/url.rs — `resolve_media_path` and `media_target_url` hold the only coordinate common to us and to any foreign side — the address of the material. There is neither a reel name nor a source timecode beside it: the timecode we subtract on reading and return on writing (`convert.rs:1255`, item 180), but between these two points it exists nowhere — neither in the model nor on the screen. That is, we have a surrogate of the reel, but we do not have the clock by which the industry checks the return.

### What already exists

- `src/libs/kt-otio/src/url.rs` — `resolve_media_path` / `media_target_url` / `relativize`:
  a relative path with POSIX slashes, the fallback `file://`; t147.
- `src/libs/kt-otio/src/convert.rs` — the starting timecode of the material (item 180):
  the start of `available_range` is subtracted on reading and added on writing,
  therefore a foreign file is read by another editor exactly as it arrived. This is
  precisely the value by which ColorTrace conforms — and with us it lives
  only inside this function.
- `src/apps/kt-app/src/util.rs` — `format_timecode`, and it also shows the scale of the gap: across the whole GUI this function has ONE call —
  `src/apps/kt-app/src/toolbar.rs`, the position of the playhead on the timeline. The timecode
  of the MATERIAL is shown nowhere.
- `src/libs/kt-media/src/placeholder.rs` — `PlaceholderSource`: a missing source does not break the project, it holds the place, and replacing the source returns the edit to the real file (t209/t210/t211). This is half of the acceptance of a return —
  the half where FILES arrive.
- `src/libs/kt-model/src/project.rs` — `MediaRef { id, path }`: the material
  has a path and nothing more.

### What is missing

- The values "source timecode" and "reel name" of `MediaRef`, and their display
  beside the event. Without the first there is nothing with which to check the return; without the second there is nothing
  to show the person when the check did not converge.
- The concept of an "event list" on INPUT. Today one can open a project (`.otio`) and
  import a legacy file (`.ktproj`) — there is no third kind of reading, "accept properties onto an
  existing edit without touching the structure", neither in the model nor in the commands.
- The very subject of the return: we have no colour at all (F-CLR-001, F-CLR-002 — both
  `no`). Therefore "give to colour" is not a luxury for us, but the only way to colour — and at the same time what we cannot either give or accept.
- There is nothing to glue the return to, for an internal reason as well: `ClipId`
  is issued as "maximum + 1" and is reused after deletion
  (`src/apps/kt-app/src/app.rs:3110`), so it is suitable as a key inside one
  document and in no way between two.

### What gets in the way

Nothing constructive in the first half: the source timecode already travels in the file,
bringing it to the model and to the screen is one stage of work. The second half runs up against not the code but the absence of a subject: there is nowhere to accept
colour properties while there is no colour. Therefore the note deliberately stops at the coordinate and does not design a receiver.

### In the `collab/` category — two more mandatory lines (rule 13)

OFFLINE: DOES NOT REMAIN — `src/libs/kt-otio/src/url.rs:73` — the absence is visible right here: the only thing we present to the foreign side is the path to the material, and the clock by which the industry checks the return exists with us only for the time of reading the file and does not survive into the model; today the editor checks the match of a shot by eye, from the picture.

NETWORK COST: to glue a returned list of events to today's edit — shared clock — the industry takes it not from wall-clock time and not from a server, but from the material: they compare the embedded timecode and the reel name, and without this value the match of a shot is expressed by nothing but a person's opinion.

NETWORK COST: to learn that the edit has changed while the colour was going on — edit log — our undo is snapshots of the whole project with a depth of 80; an edit has neither a name nor a base revision, therefore there is nothing with which to answer "what changed since the last return", even for oneself.

### Which invariants it touches

None of the twelve. Only the rule of the crate concerning the starting timecode of the
material is touched (`src/libs/kt-otio/CLAUDE.md`, item 180): the `source_in` of the model is counted
from the first frame of the file, and if the timecode of the material becomes a value of the model, both
ends of this rule will have to be named explicitly, so as not to create a second coordinate system.
The fifth rule of the UI checklist is also nearby: a value that exists only
inside one function cannot be shown, and show it we will have to.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: the source timecode and the reel name of `MediaRef`, their output beside the event |
| Where the main work is | kt-model (`MediaRef`: the starting timecode and reel), kt-otio (stop discarding both on reading), kt-app (showing the timecode of the material — today `format_timecode` is called from exactly one place) |
| Project format | does not touch it: both values already travel in `available_range` and in the metadata of the material; only their retention is added |
| Tests to write | a round "a file with a non-zero starting timecode → model → file", with a check that the value reached the model, not merely returned to the file; a layout test of the timecode line of the material; immobility of t147 and the golden fixtures |
| Depends on | F-COL-007 (the same timecode is needed to write EDL), F-MED-005 (relinking), F-CLR-001/F-CLR-002 (the subject of the return) |
| Breaks compatibility | no |

## Verdict

We take the first half — the coordinate — and do not introduce the second. The first half
is cheap and useful alone: the source timecode and the reel name must become values of the model and appear on the screen. Today the timecode exists with us
exactly for the time of reading and writing the file (`convert.rs`, item 180), and `format_timecode`
across the whole GUI is called from one place — with the position of the playhead. This same value is
required by F-COL-007, so that there is at least something to write EDL with — that is, the stage
pays for itself twice.

The second half — the receiver of a returned list of properties — we do not introduce, and the reason is named directly: **there is no participant**. The mechanism of the industry is built entirely on there being, on the other side, a person with another programme and their own document; introducing a receiver before such a person appears means introducing a state that nobody can verify — the same formula as F-PRJ-003 and F-PRJ-001. To this is added its own reason: there is nowhere to accept colour properties while there is no colour
(F-CLR-001 and F-CLR-002 — both `no`).

How this note differs from F-PRJ-003. That one is about the portability of OUR OWN
project: what will travel silently together with the document to a second machine, and what will not
turn up there for me. Here everything is the other way round: the document neither travels nor returns,
a FOREIGN result returns as a separate list, and the question is not "what was lost in
the move", but "by which key to glue this to the edit, which has changed over the week".
F-PRJ-003 answers about the content of the move, this one — about the coordinate of the meeting.

As a separate line — the conclusion that is worth taking beyond the topic: **the industry got pessimistic locking for free, by agreement**, and its unit of
occupancy is the whole edit, exactly like the Final Cut lock on the whole
library. Any future proposal of "let's make a lock" must first explain how it is better than a promise, which costs zero lines.

Boundaries: what we give the edit outward — F-COL-007; what a foreign file loses inside us —
F-COL-008; relinking and gathering of material — F-MED-005; the portability of one's own
project — F-PRJ-003.

## Sources

- The service registry: `docs/features/_services.md`. The tracker registry and the counting method:
  `docs/features/_trackers.md`; the number is reproduced by
  `python3 src/scripts/tracker-rank.py --query '\b(conform|reconform|re-conform|color ?trace|cdl|ale)\b'`
  (export date 08.09.2026, `otio` — 680 entries, `olive` — 1853).
- https://www.provideocoalition.com/solutions-to-resolve-part-2-accurate-shot-match-in-colortracetm/
  — David Hover, 15.10.2018: how ColorTrace matches shots and why, in the file
  world, one has to make a reel out of the path to the file (accessed 2026-09-08)
- https://creativecow.net/forums/thread/how-do-timelines-get-updated-when-the-edit-changes/
  — the thread "How Do Timelines Get Updated, When The Edit Changes After Grading
  Starts?", quotes of 15.01.2015 and 25.01.2015 (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1050 — ALE
  adapter round-trip CDL Values, open since 24.08.2021 (accessed
  2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/619 — CDL
  is not unified among adapters (accessed 2026-09-08)
- Our code, checked by reading and grep on 08.09.2026:
  `src/libs/kt-otio/src/url.rs`, `src/libs/kt-otio/src/convert.rs`
  (`active_available_range`, item 180), `src/apps/kt-app/src/util.rs`
  (`format_timecode` — the only call is in `toolbar.rs`),
  `src/libs/kt-model/src/project.rs` (`MediaRef`),
  `src/libs/kt-otio/CLAUDE.md`.
- The table "Who has it and how" is compiled from the vendors' documentation, the Resolve manual
  and forum threads; not verified live — there is no public tracker for any of the five. The Premiere row is marked `[NOT VERIFIED]`: the primary source from Adobe
  (`helpx.adobe.com`) is unavailable on the date of verification; CDL support in Lumetri
  is confirmed only by secondary sources.
