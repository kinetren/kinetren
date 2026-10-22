---
id: F-COL-007
title: How exactly edits are handed off: AAF, OMF, EDL, XML
status: researched
priority: C
cost: M
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-model, kt-cli]
invariants: [9]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-PRJ-003, F-RES-003]
trackers: [otio, shotcut]
services: [team-projects, bm-cloud, resolve-project-server]
checked: 2026-09-08
---

# F-COL-007 — How exactly edits are handed off: AAF, OMF, EDL, XML

## Essence

The edit is finished, and it must be handed over: to the sound shop — for mixing, to the colourist — for colour, to graphics — for titles. The programme cannot be handed over whole: the receiving side has a different programme. Therefore the edit is exported to an intermediate format — AAF, OMF, EDL or
XML, — and the question is not which of them is "better", but which of them the receiving side will read in full. The giving side chooses not richness but the predictability of loss.

## Who has it and how

The column "How it works" answers one question: what the editor releases outward and
what must remain inside at the same time. The fate of a foreign file that has come inside is the subject of F-COL-008; the return from the receiving side — F-COL-009.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Save As → EDL Text File / AAF; File → Export → Final Cut Pro X (.fcpxml) | gives out three FOREIGN formats and none of its own: `.veg` is binary and read by nobody, so exchange for Vegas is always a degradation, not a copy of the file. The exchange lives in the "save as" dialogue, that is, it looks like saving the project and is not one |
| Adobe Premiere | 26.x | File → Export → EDL / OMF / AAF / Final Cut Pro XML | four channels, and each is assigned to a shop: OMF and AAF — to sound, EDL and FCP7 XML — to picture. The FCP XML format is named after a programme that no longer exists (Final Cut Pro 7), and it is the only XML that Premiere reads: it does not open `.fcpxml` of the modern Final Cut |
| DaVinci Resolve | 21 | Timeline → Import / Export: EDL, XML, FCPXML, AAF, DRT, OTIO/OTIOZ | the only one that reads and writes the SAME set in both directions, and the only one of the five that since 18.5 supports OTIO natively. Hence the practical conclusion, which makes our position: our native file opens in Resolve without a converter |
| Final Cut Pro | 12.3 | File → Export XML (`.fcpxml`) | exactly one format, its own. There is no AAF, no OMF, no EDL at all — the sound shop must buy a third-party converter to get a session. The most closed end of the scale: the single exit outward is described by the vendor, not by the industry |
| After Effects | 26.x | Import → Pro Import After Effects (AAF, FCP7 XML); Dynamic Link | does NOT GIVE OUT the edit at all — it only accepts, and even that via a separate importer that came from the Automatic Duck plugin bought by Adobe. The return road is Dynamic Link, that is, not a file but a live connection of two running programmes on one machine |

**Outside the core.** Without OpenTimelineIO this note would not exist: the five core editors have
exchange formats, but NOT ONE of them formulates what it pays with — while the library formulates it in one line of documentation and keeps one adapter per
format, that is, it is the only place where the losses of different formats are comparable with each other.

| Programme | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | adapters `cmx_3600`, `fcp_xml`, `fcpx_xml`, `aaf`, `ale`, `otio_json` | each format is a separate adapter over one model, and the documentation states the price directly: `.otio` is declared the only format WITHOUT losses, the others are "lossy … only store and retrieve features that are supported by that format". Exchange here is not a property of the format but a property of the PAIR "format + adapter implementation" |

## The trick behind it

**Exchange formats are chosen not by richness but by what the receiving side MUST
lose — and the poorest format turns out to be the most reliable precisely because of its poverty.** CMX3600 EDL carries exactly four things: the event number, the reel name, the source timecode and the record timecode. No effects, no levels, no clip names, no nesting. This is not a shortcoming of a 1970s format that has survived to 2026 by inertia: it is the only format of the industry that a person can READ
WITH THEIR EYES and check line by line, and a machine can check in full without interpretation.
A verifiable conform is possible only where there is nothing to interpret: as soon as
effects get into the channel, "it matched" ceases to be a boolean and becomes
a subject of dispute between two implementations. Therefore in serious deliveries EDL travels not
INSTEAD OF AAF, but TOGETHER with it — as a checksum for the rich channel.

**The second detail, visible only through failures: AAF and OMF carry MEDIA INSIDE THEMSELVES, and
therefore they have a limit at which they do not degrade but crash.** OMF is a
format with 32-bit offsets, hence the ceiling of 2 GB per file, against which the delivery of
sound breaks exactly when the project has grown to something interesting. The rich channel breaks
not on the complexity of the edit but on the volume of embedded sound; the poor channel has no such limit
at all, because it carries not a single byte of material. Hence a practice that looks like superstition from the outside: delivering sound not
as one AAF, but cut into reels.

**Third: we have no exchange format — because it coincided with our native one.** Our project
is `.otio`, that very format which the reference library calls the only lossless one, and which Resolve 21 opens natively since version 18.5.
We do not "know how to export to OTIO" — we have NOTHING to export from. This is a rare case
where we are structurally ahead of the five, and it cost us not work but a single decision, R.24: not to create our own container. For all five the native
format is not portable (`.veg` is binary, `.prproj` is binary, the FCP library is a package with a
database, the Resolve project is a row in PostgreSQL), therefore each of them pays for exchange with a
separate channel, and we pay zero.

## Pros

- Giving the edit outward from us = copying the project file. No export dialogue, no choice of
  format, no question "what will fall off": the document has one kind, and it is the one that travels.
- Nothing is lost on giving out CONSTRUCTIVELY, not by effort: everything we do not understand in a foreign
  file we store verbatim (`RawObject`, `extra`), therefore our output is not poorer than our input.
- The receiving side can be stricter than we are: `.otio` is a text JSON, and it can be read through by eye just like EDL. Poverty as verifiability we have achieved not through poverty but through readability.
- Paths to the material are written relative (`src/libs/kt-otio/src/url.rs`), so the
  folder "project + material" travels whole and opens on the other side without relinking.
- Of the five formats of the industry, one — OTIO — we support more fully than any of the five editors: for them it is an adapter over their own model, for us it IS the model,
  and the write loop is idempotent byte for byte (t144).

## Cons

- Our advantage works for exactly one addressee. Of the five core editors only Resolve reads
  `.otio`. Vegas, Premiere, Final Cut and After Effects will not open our project with anything at all — and this is not "the adapter has not been written yet" but a property of their format: exchange with them requires exactly that poor channel which we do not have.
- We do not write EDL, and EDL is more needed than AAF. The only format that any shop at all will read is the one we have not implemented; and it is also the one whose implementation is cheaper than all the others together (text, four fields).
- Neither the reel name nor the starting timecode of the material is output outward. We subtract the starting timecode on reading and return it on writing (item 180), but the model lives on `source_in` from the first frame of the file, not on the timecode — that is, the key by which the receiving side conforms is not present in our project as a value.
- We cannot give the SOUND separately by any means. AAF/OMF with embedded sound is an encoding of the material, that is, a job of export; today export can do one kind of task (the whole project / a region) and one container.
- Our `.otio` carries our data in `metadata["kt"]`, and a foreign reader will ignore it — this is correct and is designed so, but it means that the edit given out is poorer for the recipient by exactly the morphers, cameras and automation. We lose nothing on giving out; the recipient loses, and there is nothing with which to warn them.

## What users say

SIGNAL: exchange formats — 92 tasks in otio, `\b(aaf|omf|edl|cmx.?3600|fcpxml)\b`; share 13.5% against 0.4% in shotcut

The number in itself is not a finding, but its distribution is. 13.5% of all tasks of the OTIO tracker concern exchange formats, and this is an order of magnitude more than for any
editor: shotcut 0.4%, losslesscut 0.4%, flowblade 0.1%, olive 0.1%,
openshot 0.0%, pitivi 0.0%. The interpretation is exactly one, and it is unpleasant for the
industry: **exchange is not a topic of an editor, it is a separate product.** People write
almost nothing about exchange in an editor not because it works, but because the pain goes
to where the adapters live — and there it makes up a seventh part of the whole tracker.
The core five have no public trackers at all (`docs/features/_trackers.md`),
so their share cannot be compared with anything; but the very fact that outside the editors this share is thirty times
higher than inside them shows where the work flows.

The rich channel breaks on volume, not on complexity:

> any OMF that went of 2gib would stall and crash and simple not export at all
> — eoghans21094336, 27.07.2015,
> https://community.adobe.com/t5/premiere-pro-discussions/omf-aaf-export-does-it-ever-work-correctly/td-p/3177642

And it breaks partially, which is worse than a failure — because the giving side learns of it from the receiving side, not from the programme:

> AAF export finishes but lists several errors and less than half of the audio is included
> — Anonymous, 23.01.2011,
> https://community.adobe.com/t5/premiere-pro-discussions/omf-aaf-export-does-it-ever-work-correctly/td-p/3177642

The price of poverty is named in the library's documentation in plain text, and this is a rare
case where the vendor writes about losses in the section "how to write an adapter", not in
the section "features":

> The '.otio' format is the only format that is lossless. It can store and retrieve all of the objects, metadata and features available in OpenTimelineIO.
> — OpenTimelineIO, "Writing an OTIO Adapter", version 0.18.x, accessed 08.09.2026,
> https://opentimelineio.readthedocs.io/en/latest/tutorials/write-an-adapter.html

## How it fits Kinotrain

ADDRESS: src/libs/kt-otio/src/schema.rs — the dispatcher `decode_composable` and its passthrough fallback are precisely our "exchange format": everything we do not understand remains `RawObject` and travels back verbatim, therefore the file we give out is never poorer than the one accepted. It is exactly this property that makes superfluous a second export channel, because of which for the five core editors exchange formats are a separate subsystem with a separate list of losses.

### What already exists

- `src/libs/kt-otio/src/schema.rs` — typed schemas plus passthrough:
  unknown FIELDS of known schemas in `#[serde(flatten)] extra`, objects of
  unknown schemas in `RawObject` verbatim, canonical key order.
- `src/libs/kt-otio/src/convert.rs` — `Retained` as the storage of everything foreign:
  extra and metadata of the timeline, stack, tracks, gaps, transitions and clips;
  clips are bound by `ClipId`, the rest by position (a documented
  limitation of v1).
- `src/libs/kt-otio/src/url.rs` — `media_target_url`/`resolve_media_path`:
  a relative path with POSIX slashes, the fallback is `file://` with percent-encoding, as
  Resolve and Hiero write.
- `src/libs/kt-otio/src/meta.rs` — the version by CONTENT: a file without our innovations
  remains v1 and is read by any build.
- Tests: `libs/kt-otio/tests/format.rs` (t144 idempotence, t145
  passthrough, t147 paths), `libs/kt-otio/tests/convert.rs` (t154 — nine
  foreign upstream samples are read and re-saved idempotently),
  `libs/kt-otio/tests/otio_conformance.rs` (t149 — checked against the real
  library, gated on the Python opentimelineio), 
  `apps/kt-cli/tests/cross_render.rs` (t158 — our file is rendered by a FOREIGN
  engine via melt).

### What is missing

- A writer of CMX3600 EDL. It is four fields of text and the only format that any shop will
  read; unlike AAF it requires neither a library, nor a
  binary container, nor embedding of material.
- A value "source timecode" in the model. Today it exists only inside
  `src/libs/kt-otio/src/convert.rs` for the time of reading and writing (item 180): subtracted on
  input, added on output, and between these — `source_in` from the first frame of the file.
  There is nothing from which to write EDL while this value exists neither in the model nor on the screen.
- A reel name. `MediaRef` has a path and nothing else; the `reel name` of a foreign file we do not even read.
- Warnings for the recipient: our file carries morphers, cameras and automation in
  `metadata["kt"]`, and a foreign editor will silently not show them.

### What gets in the way

AAF is a binary container, and writing it means a new dependency: either a
vendored library, or our own implementation of Microsoft's structured storage. This is incommensurable with the
cost of EDL and does not pay off in a single live scenario. OMF additionally requires embedding the sound itself in the file — that is, encoding the
material, and that is a job of export, not of the project format.

### In the `collab/` category — two more mandatory lines (rule 13)

OFFLINE: REMAINS — `src/libs/kt-otio/src/schema.rs:426` — giving the edit outward for us means copying the project file, and this works on a flash drive exactly as it does through a server: what was not understood is stored verbatim, therefore our output is never poorer than our input, and the addressee with Resolve needs no converter at all.

NETWORK COST: for the file given out to open non-empty at the receiving side, the same material must lie there — the media address — we write the path relative to the project, but there is no means on the other side to verify that the material is there at all: there is no reel name and no source timecode in the model, and the hash lives in the local index database and does not travel outward.

NETWORK COST: giving the sound as a separate file with embedded material (AAF/OMF) — invariant 9 — is encoding, that is, a job of export, and our export is in-process and knows exactly two tasks: the whole project and a region.

### Which invariants it touches

Invariant 9 (export is fully in-process) — the only one, and only for formats with embedded material: AAF and OMF would require encoding the sound into an exchange container, that is, opening a third kind of export task. EDL, XML and OTIO do not touch any invariant: these are reading and writing text in kt-otio.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: the source timecode in the model and on the screen, then the writer of EDL |
| Where the main work is | kt-model (the value of the source timecode and the reel name in `MediaRef`), kt-otio (the CMX3600 writer), kt-cli (the output key) |
| Project format | does not touch it: the source timecode already travels in `available_range`, nothing is added to `metadata.kt` |
| Tests to write | a round "our EDL → the reference library → the same list of events"; an EDL with a fractional fps and with a non-zero starting timecode of the material; immobility of t144/t148 when the reel name appears |
| Depends on | F-PRJ-003 (portability of one's own project), F-RES-003 (versions of the format); borders on F-COL-008 and F-COL-009 |
| Breaks compatibility | no |

## Verdict

We do not take it — and this is the rare case where "we do not take it" means "we are already ahead".
We do not introduce exchange formats, because our exchange format is the native one:
`.otio` is declared by the reference library the only lossless one, is read natively by Resolve,
and is checked by us with a round test on nine foreign samples. There is nothing to lose,
and nothing to simplify either — while all five core editors have a native format
that is not portable, and exchange costs them a separate subsystem with a separate list of losses.

From this note we take exactly one conclusion and one piece of work. The conclusion: **poverty can be a property, not a defect** —
CMX3600 is reliable because there is nothing to interpret differently in it, and any future proposal of "let's give out
richer" must first answer how the receiving side will verify that it matched.
The work: the source timecode must become a value of the model and appear on the screen.
Today it exists only inside `src/libs/kt-otio/src/convert.rs` for the time of
reading and writing, and this is the only coordinate on which we and the foreign side speak the same way; without it neither can EDL be written, nor
can a return from the colourist be accepted (F-COL-009).

Boundaries: the fate of a FOREIGN file inside us — F-COL-008; the return from the receiving side —
F-COL-009; the portability of one's own project to a second machine — F-PRJ-003; the versions of
our format — F-RES-003.

## Sources

- The service registry and the unit of occupancy: `docs/features/_services.md`. The tracker
  registry and the counting method: `docs/features/_trackers.md`; the number is reproduced by
  `python3 src/scripts/tracker-rank.py --query '\b(aaf|omf|edl|cmx.?3600|fcpxml)\b'`
  (export date 08.09.2026, `otio` — 680 entries).
- https://opentimelineio.readthedocs.io/en/latest/tutorials/write-an-adapter.html
  — "The '.otio' format is the only format that is lossless" and the declaration that the other formats are lossy (accessed 2026-09-08)
- https://opentimelineio.readthedocs.io/en/latest/tutorials/otio-serialized-schema.html
  — the composition of the schemas and their current versions (accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/omf-aaf-export-does-it-ever-work-correctly/td-p/3177642
  — the thread "OMF/AAF export: Does it ever work correctly?", quotes of 2011 and 2015
  (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/168 — EDL
  clip names as reel names: the reel name as the only key of a conform (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/766 —
  "Creating AAF from OTIO does not work when imported into DaVinci Resolve": the adapter and the receiver
  diverge in their reading of one format (accessed 2026-09-08)
- Our code, checked by reading on 08.09.2026: `src/libs/kt-otio/src/schema.rs`
  (`decode_composable`, `decode_marker`, `RawObject`),
  `src/libs/kt-otio/src/convert.rs` (`Retained`), `src/libs/kt-otio/src/url.rs`,
  `src/libs/kt-otio/CLAUDE.md`, the fixtures `src/fixtures/otio-samples`.
- The table "Who has it and how" is compiled from the vendors' documentation and the composition
  of the dialogues (Save As → EDL/AAF in VEGAS, File → Export in Premiere, Timeline → Import/Export
  in Resolve, Export XML in Final Cut, Pro Import After Effects in AE); not verified live — there is no public tracker for any of the five. The OTIO support
  in Resolve from 18.5 and the absence of native AAF in Final Cut are confirmed by secondary
  sources; the primary source, Blackmagic (`forum.blackmagicdesign.com`), is
  unavailable — Cloudflare.
