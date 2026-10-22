---
id: F-COL-003
title: "Not arrived yet" is not "lost"
status: researched
priority: B
cost: M
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-media, kt-model, kt-app, kt-engine]
invariants: [5]
ui_rules: [16]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud, team-projects, syncthing]
checked: 2026-09-08
---

# F-COL-003 — "Not arrived yet" is not "lost"

## Essence

The material is not on the machine — and that is NORMAL: the shoot is happening right now, the bytes
have only just started to travel, and the edit has already begun on the proxies. At this moment the
editor asks not the question he asks when the source is lost: not "how is this repaired", but "how long
to wait and has everything arrived". The action differs between the two states — waiting versus
repairing — and confusing them is costly in both directions: repairing what will arrive by itself in a
minute, and waiting for what will never arrive. Outwardly both states look the same: the clip is missing,
the frame is missing, a red plate.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is only Offline media: the file either opened or it did not. "Waiting" as a state does not exist, and there is nowhere for it to come from — VEGAS has no service |
| Adobe Premiere | 26.x | Frame.io Camera to Cloud (a panel inside the editor) + Link Media | proxies travel from the site during the shoot and land in the bin; the originals arrive later and are substituted by relink. The "in transit" state lives in the service's PANEL, while on the timeline it is the same "Media Offline" as for a lost file |
| DaVinci Resolve | 21 | Blackmagic Cloud, media sync into bins | the only one that promises on-the-fly sync of sources — "into the bin while the camera is recording"; progress is visible in the Media Pool, but until the sync finishes the clip remains an ordinary offline |
| Final Cut Pro | 12.3 | none | only Missing File — a red frame with a triangle. Apple made neither a cloud nor an arrival queue, so there is nothing to tell apart |
| After Effects | 26.x | Team Projects: media offline / placeholder | someone else's edit references material you do not yet have; "not downloaded" is shown with the same placeholder as "lost", and the difference is visible only in the Team Projects panel |

There is no second table here (software outside the core): the services for which the "in transit" state
appeared at all (`frame-io`, `bm-cloud`, `team-projects`) live in `_services.md`, and under the doctrine of
the three registries they are not admitted into the verdict table — they explain the cost, not what we are
compared against. The mechanism is nevertheless described honestly from the core's lines: two editors have it
as a panel inside the program.

## The trick behind it

**In a shared scheme the EDIT STARTS BEFORE the material, and this reverses the order of things: the link
appears before the thing it links to.** The project is kilobytes and travels instantly; the material is
gigabytes and travels for hours; therefore offline is not an emergency state for them but an INITIAL one,
through which every frame of every project passes. A consequence follows that is absent from every list of
features: the "proxy" in this scheme is not about the machine's performance but about the ORDER OF ARRIVAL;
the light copy arrives first not because the editor needs a light file, but because the heavy one has not
yet arrived.

**The substitution condition explains how "in transit" differs from "lost".** Frame.io requires that the proxy
carry the same clip name and the same timecode as the original — otherwise there is nothing to substitute the
original with later. That is, "not arrived yet" is a state with a KNOWN name, a known start and a known duration,
while "lost" is a state where nothing is known except the path. The difference between them lies not in the
wording of a message but in the presence of an EXPECTATION, and an expectation is data: the list of what must
arrive exists before the bytes themselves.

**And exactly this difference is absent from the timeline for everyone.** All five editors draw one and the same
plate; the difference lives in ANOTHER window — in the service's panel, where progress is shown. The practical
consequence is visible only once one has worked a while: the editor looking at the timeline cannot tell "arriving
right now" from "lost for good", and is forced to keep a second panel open for the sake of one bit of state. A
twice-reinvented error: the distinguishing mechanism has been built in Premiere and Resolve, but it has not been
brought to the place where the person makes the decision.

## Pros

- The order "document first, bytes afterwards" makes it possible to start work at the moment when it could not
  physically have been started: the edit runs while the shoot is still going.
- An expectation expressed as data gives a second thing for free — a CHECK OF COMPLETENESS: the list of "what must
  arrive" answers the question "is everything in place" without walking the disk.
- Substitution by name and timecode requires neither a database nor a server: the rule "name and time match — the same
  material" works even between two folders on one machine.
- The "in transit" state corrects itself: unlike "lost", it requires no action from the person at all, if nothing
  has broken.

## Cons

- A wait is a promise, and promises are not kept: the class "hangs on the first file" ("Syncing 1 of N", zero progress for hours) is described in detail by users of Blackmagic, and it cannot be told apart from a normal wait by anything except patience.
- Substitution by name and timecode is just as blind: two takes with the same name from different folders differ only by which one arrived first.
- A double set of material (proxy and original) doubles the bookkeeping: switching back and forth breaks exactly where the names diverged by one character, and conflict suffixes (`-conflict`) multiply silently.
- The state lives in the service's panel, so it vanishes together with the subscription: what looks in the project like a property of the clip is in fact a property of someone else's host.

## What users say

The substitution condition is the most substantive phrase in the whole documentation of the class, because it describes not a capability but a REQUIREMENT for something that does not exist yet:

> They will have the same clipname and timecode as the original file, which is
> necessary for switching to the originals later on.
> — Frame.io, "C2C: Complete Proxy Workflow Guide", updated 25.03.2025,
> https://help.frame.io/en/articles/6079079-c2c-complete-proxy-workflow-guide

Blackmagic's promise is the only claim in the industry that material arrives in the bin DURING work, not before it:

> The ability to transfer media directly into the DaVinci Resolve media bin as
> editors are working is revolutionary.
> — Blackmagic Design, DaVinci Resolve collaboration page (accessed 08.09.2026),
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration

NO QUOTES: a live user quote about a stuck sync could not be obtained. This is a limit of the platforms, not an absence of the class. The thread "Davinci Resolve cloud sync doesn't work" on `forum.blackmagicdesign.com` (t=191343) is visible through search: in the snippet people describe files "Queued for upload" while the client shows "All your files are up to date". But the forum itself is closed by Cloudflare, and a verbatim quote with a nickname and date cannot be taken from it (checked 08.09.2026, `_services.md`, "Availability of sources"). Reddit is on the fetcher's block-list, and `helpx.adobe.com` is unavailable. A search of `community.adobe.com` turns up threads about unfinished UPLOADS to Frame.io, not about waiting for material on the editor's side. So both quotes above are vendor quotes, and this is stated plainly.

## How it fits Kinotrain

### What already exists

- `src/libs/kt-media/src/placeholder.rs` — `PlaceholderSource`: a dark frame with the file's NAME, silence in the audio, a duration of 1000 hours, `fps: None`. The stub is an ordinary `MediaSource`, so it travels the common path (preview, prefetch, cache, FX, export) and does not touch invariant 5.
- `src/apps/kt-app/src/util.rs` — `spawn_media_watchdog`: a separate thread every 900 ms walks all paths of the project and puts the unavailable ones into a shared `HashSet`. A WAITING mechanism is thus already present with us, only it is binary.
- `src/apps/kt-app/src/app.rs` — `poll_returned_sources`: a file that has returned is opened in the background and replaces the player's source on the fly (`player.replace_media`); the position and transport are not touched. Covered by tests t211 (model replacement of the source) and t209/t210 (opening and rendering with stubs). That is, the second half of the story, "the bytes have arrived", works with us better than in three of the core editors.
- `src/libs/kt-model/src/project.rs` — `MediaRef { id, path }`: exactly two fields.

### What is missing

- The state itself. We have two, "opened" and "did not open", and we need three. The third differs not in its text but in the fact that the person's action is "do nothing".
- Expected properties of the material. `MediaRef` carries the path and nothing else: an arrived file has nothing to be compared against, so "the wrong thing arrived" is indistinguishable from "the right thing arrived".
- A distinction between "the file appeared" and "the file has finished being written". The watchdog counts a file as returned by `Path::exists()`, but a file still being copied exists from its first byte: arrival is declared too early and we open a truncated file. The only saving grace is that after the write finishes, size and mtime change and the records are rebuilt (`ensure_media`, F-COL-002), but one wrong frame in the preview has already been seen by the person. What happens NEXT with a growing file and with stale memory of it is the subject of F-COL-004; here only the moment at which we declare arrival matters.
- A render that knows the difference. Today an export with unavailable sources honestly warns and draws stubs (t210). "Wait, it is on its way" is a different decision and a different button, and UI rule 16 requires that an impossibility be an error, not a faded hint.

### Why this note differs from F-HLP-012

F-HLP-012 ("A missing source: how the editor reports it") is about the TEXT half: how many messages there are, at which four points they appear, what is written in the stub frame, how the path is fixed. Here the subject is different and lies a floor lower: **the difference between two STATES in the model.** The test is simple: anything that can be fixed by rewriting a line of the `i18n/*.toml` catalogue goes there; anything that needs a new field or a new flag on the media goes here. The text half of this topic is handed to F-HLP-012 explicitly and in full: the wording "waiting" against "lost", their place in the status bar and in the bell, the behaviour of the render window are its section, not ours. From it we take back the conclusion that works here as a constraint: the name of the lost file is already written INTO THE FRAME, so the third state must be visible there too, not only in the panel, or we would repeat someone else's mistake, where the difference lives in another window.

### What gets in the way

A new field on `MediaRef` is a version of the `.otio` schema (today v4, `kt-otio`), and introducing one for the sake of a state that never occurs without a second participant is not acceptable. The "expected" flag by its nature does NOT belong to the document: it lives exactly as long as the copying runs, and it must not get into the project file. So its place is in the session, next to `placeholder_media` and the watchdog, not in the model. This is the real constructive point: the only thing that asks to go into the document is the expected PROPERTIES of the material (duration, size, hash), and those are meaningful on their own, with no second machine at all.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: DOES NOT REMAIN — `src/libs/kt-media/src/placeholder.rs:127` — the stub's constructor takes ONE caption string and nothing more: it has no expected duration, no size, no "copy in progress" flag, and `info()` declares video and audio existing 1000 hours ahead. One and the same frame answers both "the file is lost" and "the file is still being written", and today only a person who remembers having started the copy can tell them apart.

NETWORK COST: the side that declares in advance the composition of the shoot and the progress of each file — a user account — the list of "what should arrive" exists before the bytes and belongs to a foreign host: a file that is not there cannot report its own name, timecode and size, and to obtain them, other than by becoming a client of the service, there is no way today.

### Which invariants it touches

Invariant 5 (exactly one MediaWorker owns the decoder) is the only one, and it works in our favour here: source substitution already goes through the owning thread (`player.replace_media` over a channel from the background `reopen-media`), so the third state needs neither a new thread nor a mutex. UI rule 16 (an impossibility is an error, not a faded hint) is the boundary for a future rendering decision: "the material is still on its way" must be either an error or a deliberate choice by the person, but not a silent substitution of a stub.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: expected properties in the model and the third state in the session |
| Where the main work is | kt-model (expected properties of the material), kt-app (the watchdog tells "appeared" from "finished being written"), kt-media (a stub that knows what it is waiting for) |
| Project format | a v5 is needed if the expected properties get into `MediaRef`; the "in transit" flag never gets into the file |
| Tests to write | the watchdog does not declare a file arrived while it is still growing; the "waiting" stub differs from the "lost" stub in the frame byte for byte; an arrived file that does not match the expected properties does not silently replace the source |
| Depends on | F-HLP-012 (the text half), F-COL-004 (a growing file and stale memory of it), F-MED-005 (relinking), F-MED-002 (proxies — a "light copy" appears here for a different reason, not for speed) |
| Breaks compatibility | no |

## Verdict

We take the local core and do not introduce the server half: there is no second participant, and the note has no right to declare his appearance. Of the three parts of the foreign scheme, two are already ready for us: a stub that does not get in the way of working and rendering, and source substitution on the fly when the file has returned (t211). What is missing is exactly what makes sense even alone: **the expected properties of the material** (duration, size, hash) in `MediaRef`, and a distinction in the watchdog between "the file appeared" and "the file is finished being written". The first turns opening a project into a completeness check; the second closes today's defect: we declare arrival by `Path::exists()`, that is, by the first byte of the copy, and we build peaks from a truncated file.

The "expected" flag we do NOT put into the project file: it lives as long as the copying does, and its place is the session.

Boundaries: how the program will SAY that a source is missing is F-HLP-012 (its text half was handed over to it explicitly); a file that is still being WRITTEN and stale memory of it is F-COL-004 (there the subject starts after the file has opened, here before); a proxy as a served derivative is F-COL-005; how a path is repaired is F-MED-005; the service base next to the project is F-COL-002; the shared volume and the protocol requirements are F-COL-001.

## Sources

- https://help.frame.io/en/articles/6079079-c2c-complete-proxy-workflow-guide —
  Frame.io, "C2C: Complete Proxy Workflow Guide", updated 25.03.2025 (accessed
  2026-09-08).
- https://help.frame.io/en/articles/8896457-c2c-getting-started-with-camera-to-cloud
  — how C2C connections are set up and the order in which proxies arrive (accessed
  2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  Blackmagic Design, the promise of syncing media into bins during work (accessed
  2026-09-08).
- Thread `forum.blackmagicdesign.com` t=191343 "Davinci Resolve cloud sync doesn't
  work" — visible only as a search snippet, not verbatim-accessible (Cloudflare), so it
  was not taken as a quote; see the note `NO QUOTES:` in the feedback section.
- Service registry, the unit of occupancy and availability of sources —
  `docs/features/_services.md`.
- Our code, checked by reading on 08.09.2026: `src/libs/kt-media/src/placeholder.rs`,
  `src/apps/kt-app/src/util.rs` (`spawn_media_watchdog`, `open_source_or_placeholder`),
  `src/apps/kt-app/src/app.rs` (`poll_returned_sources`, `placeholder_media`),
  `src/libs/kt-model/src/project.rs` (`MediaRef`).
