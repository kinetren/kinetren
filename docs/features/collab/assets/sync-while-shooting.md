---
id: F-COL-004
title: Footage synchronized during shooting
status: researched
priority: C
cost: M
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: partial
crates: [kt-cache, kt-media, kt-engine, kt-app]
invariants: [4, 5, 8]
ui_rules: [12, 20]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [bm-cloud, syncthing]
checked: 2026-09-08
---

# F-COL-004 — Footage synchronized during shooting

## Essence

The camera is still recording, yet the material is already in the bin: the editor
cuts the start of a take while its end is still being written. At that moment the
file is not being "copied" but GROWS — it has no final duration yet, and everything
the program knows about it becomes stale a second later. The same class arises
without any shooting at all: screen capture, recording from a sound card, a copy
from a memory card still in progress. The question is one — what the program
considers a file that is still being written, and what it does with what it has
already remembered about it.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | neither material sync nor growing files is claimed in the feature list; the file is read as it was when opened — **[NOT VERIFIED]** from the 2026 product line documentation |
| Adobe Premiere | 26.x | Growing Files (historically — the broadcast path MXF/OP1a) | the duration is taken from the header at import; segmented Apple AVF recording is not recognized at all — the file is shown as five seconds long until the recording ends, and the update arrives in one jump at the end |
| DaVinci Resolve | 21 | Blackmagic Cloud live sync, multi sync | the ONLY one that promises sync during recording. Mechanism: the camera writes a SECOND file — a full-size HD proxy H.264 — and it is this file that travels to the bin over the network, not the original. The original stays on the card and leaves later, through the ordinary queue |
| Final Cut Pro | 12.3 | growing files | support exists and works with segmented AVF recording — according to the evidence of the recording software vendor (ToolsOnAir), not according to Apple's documentation: no direct statement could be found in the documentation — **[NOT VERIFIED]** |
| After Effects | 26.x | none | the duration of a project item is fixed at import; a growing source is not foreseen by the composition model |

**Outside the core.** Without Syncthing it is impossible to describe what happens to
a file that CHANGES DURING transfer: this is the only place where the rule is not
implied but written down.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | conflict resolution, rescan | there is no merging in any form: the file with the older modification time loses; on a tie, the device with the larger first 63 bits of its device ID wins, and the loser is renamed to `<name>.sync-conflict-<date>-<time>-<who>.<ext>` and from then on lives as an ordinary file, spreading across all nodes |

## The trick behind it

**1. The promise "sync even while the camera is recording" is kept by the fact that
NOT the material flies over the network.** Live sync works not because they learned
to transfer a growing file, but because the new Blackmagic cameras write a SECOND
file — a full-size HD proxy H.264 — and it is this one that is synchronized. The
original goes nowhere at that moment. That is, a feature that in the feature list looks
like "transferring material on the fly" is in fact an agreement with the camera to write
a cheap copy which a) has a known bitrate in advance, b) has no index at the end, and
c) is not a pity to send again. This is exactly what explains the complaint about the
queue stuck at "Syncing 1 of 296 files (0KB of 27.13 GB)": it is the ORIGINALS that
stand still, traveling through the ordinary queue, and this queue has nothing to do
with editing.

**2. A growing file breaks not on its size but on its HEADER.** Technically it
transfers perfectly well — appending to the end is ideal for any synchronization. The
problem is that the duration is read once, from the header, and a container that is
still being written has there either zero or the length of the first segment. Hence the
literal symptom in Premiere: the file is seen as five seconds long, and the update arrives
in one jump at the end of the recording. The point is not that the editor "cannot keep up",
but that it does not re-ask: the duration in its model is a property taken at opening,
not a query to the file.

**3. Identity by "size + mtime" is exactly the check that ALWAYS gives "another file" for
a growing file.** Any system that caches derivatives of the material compares them by this
pair; for a file being appended it changes every second, so the derivatives are never reused,
and the honest reaction "the content has changed — recompute" turns into endless recomputation.
This is the price paid for indexing during shooting, and it is absent from the feature list.

## Pros

- The editing starts before the end of shooting — not "faster" but EARLIER: a long take
  is cut while it is being recorded, and by the end of recording the first assembly already
  exists.
- Separate recording of a proxy by the camera removes the whole transcode from the editing
  station: the light copy appears for free, because it is written by the same device as the
  original, and at the same moment.
- Appending to the end is the only operation that two-way synchronization carries without
  conflict in principle: a conflict requires TWO writers, while a growing file has exactly
  one writer.
- The class is useful even without a second machine: screen capture and recording from a sound
  card produce exactly the same kind of file, and "open what is still being written" is a local task.

## Cons

- The duration in the model becomes a TEMPORARY quantity. Everything computed from it — the end
  of an event, the length of a band, the step of a filmstrip, the prefetch boundaries — must be
  able to recompute, otherwise a growing file produces a growing mismatch.
- Derivatives (packet index, peaks, thumbnails) lose their value on every append: the identity
  check honestly says "the file is different", and the work is thrown away entirely.
- The read point at the end of a growing file is the most fragile place: in Premiere on MXF this
  gave "a lot of crashes, especially when playing near the end of the file while it grows".
- The promise holds only until the participants start waiting for the ORIGINALS: the 27 GB queue
  has not gone anywhere, it simply stands behind the scenes — and it is exactly this that the user
  sees as a frozen sync.
- The cost is checked constantly: to know that a file has grown, someone must regularly probe it —
  this is exactly the background work that must not be done during a gesture or playback.

## What users say

The mechanics of the failure are named literally, and three years later have not changed:

> Premiere is not recognising this at all and it just display the file with 5sec
> and only at the end of the recording there is an update to the file duration.
> — jamiemcgee, Adobe Community, «Premiere Growing Files AVF», 17.02.2020,
> https://community.adobe.com/questions-729/premiere-growing-files-avf-1359820

The same author, the same thread, the workaround through MXF:

> a lot of Premiere crashes particularly when playing around the end point of
> the video file as it grows
> — jamiemcgee, ibid., 22.03.2023

What could NOT be quoted: complaints about a stuck Blackmagic sync. The thread
«Warning: Blackmagic Cloud Not Ready for Professional Use»
(`forum.blackmagicdesign.com`, `viewtopic.php?f=39&t=224635`) does not open in this
environment — Cloudflare returns 403 both to a direct request and through the reader;
its content is visible only as a summary in search results, and a summary is not a quote.
Therefore the line about the queue stuck at «Syncing 1 of 296 files (0KB of 27.13
GB)», and about 200+ files with the suffix `-conflict`, is given as a FACT from a
secondary source, without an author name or date, and no conclusion of this note is built
on it. Blackmagic's wording about sync during recording is taken from the primary source
(see "Sources") and is quoted in the table, not here: it is the vendor's promise, not a review.

## How it fits Kinotrain

### What already exists

- **The "file changed" mechanism is written and debugged** — but it has no source of
  events. `BurstCache::invalidate_source` (`src/libs/kt-cache/src/lib.rs`) drops all
  entries of the source and its coverage; exactly one place of product code calls it —
  `Player::replace_media` (`src/libs/kt-engine/src/player.rs`), that is, a person who
  returned a missing file (t211). The thumbnail service keeps its half:
  `ThumbService::invalidate_media` (`src/apps/kt-app/src/thumbs.rs`) resets the negative
  caches and is called from the bin card and from `add_media`.
- **File identity is already computed correctly.** `ensure_media`
  (`src/libs/kt-media/src/index.rs`): if size and mtime match — the fast path; if not —
  a full xxh3 over the file; if the content differs — `purge_media` removes for this media
  `packet_index`, `audio_peaks`, `thumbnails` and `poster`. The rule is honest, and on a
  growing file it fires on EVERY opening.
- **The duration is asked once and remembered together with the failure.**
  `durations: HashMap<MediaId, Option<Tick>>` in the thumbnail service: `None` means
  "tried, did not work", and there will be no repeated request (t69). For a file that was
  still being written at the moment of the query, this literally means "remembered as ready" —
  or as hopeless.
- **A placeholder in place of a missing file exists** (`PlaceholderSource`, t209), and
  swapping the source on the fly without losing the position and FX tails is debugged
  (`replace_media`, t211). This is half of the machinery of "the file under us has changed".

### What is missing

- Not a single watcher and not a single read of mtime anywhere in `kt-app`: the program
  physically cannot learn that a file has grown until it is asked to.
- A repeated polling of the duration. Today `info().duration` is read when the source is
  opened and lives until the end of the session.
- A notion of "the file is still being written". `MediaEntryStatus` knows `Reused` and `Rebuilt`,
  and the entry has a `complete` flag — but it concerns the completeness of the INDEX, not the
  completeness of the file.

### What gets in the way

We have no cheap "has it not grown?": the identity check on a changed size goes into a full
hash of the file, and then into `purge_media` — that is, each append costs a read of the whole
file and the loss of all derivatives. Second: the duration is not only a property of the decoder
but also what the length of an event in the model is computed from; the event cannot be silently
stretched (UI rule 20 — nothing grows by itself), and not stretching it means showing the user
a file that is too short. The only honest form of the local remainder: update what has been
DECODED, and do not touch what lies in the document.

OFFLINE: DEGENERATES — `src/libs/kt-cache/src/lib.rs` — sync on the fly degenerates into a manual command: "the file has changed on disk" is expressed in our code by exactly one function, and the only place of product code that calls it is the media replacement at a person's request. Until someone asks, coverage honestly shows as warm the bursts decoded from the previous, shorter version of the file, and the entries pinned to the loop region survive even a shortage of memory.

NETWORK COST: to distinguish a file being appended from one already appended — shared clock — our identity is size and mtime, and the mtime of a growing file comes from the clock of a FOREIGN machine: without agreed-upon time "changed just now" and "appended just now" are the same record, and both are read as "ready".

NETWORK COST: a duration that grows — invariant 5 — the decoder is owned by exactly one MediaWorker, and the duration is read when the source is opened: there is no honest answer to "how long is it now" in the process anywhere except in this worker, and today there is no one and no way to ask it.

### Which invariants it touches

- **No. 4** (the cache is the single truth about coverage): a growing file is the only case where
  this truth becomes stale not from eviction but from outside. It is cured only by
  `invalidate_source`; one must not set up a second bookkeeping "this piece is already stale" — that
  is the very parallel truth whose prohibition the invariant was written for.
- **No. 5** (one MediaWorker owns the decoder): re-asking the duration must be done by it too, via the
  priority queue, and not by a second short-lived source.
- **No. 8** (scrolling/zooming does not generate decode requests): polling for growth must go the same
  way as thumbnails and waveforms — through the sidecar DB and requests to the worker — and not as
  "let us look at the file size on every UI frame".
- UI rules: **12** (background work does not get in the way — the polling is not done during a gesture
  or during playback), **20** (nothing grows by itself — the DECODED part is updated, the event length
  in the document is not touched without a person).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: a request "re-check the duration" into the worker's queue, and an honest cache reset based on its result |
| Where the main work is | kt-media (a cheap growth check without a full hash), kt-engine (the request to the worker and `invalidate_source` based on its outcome), kt-app (who sets it and when) |
| Project format | not touched: "the file is still being written" is session state, not document state |
| Tests to write | a synthetic source whose duration grows between openings: after the update the coverage does not show as warm what was not there; the event length in the model is NOT changed; a repeated request is not issued more often than once per interval |
| Depends on | nothing; neighboring — F-MED-005 (the media address), F-COL-005 (derivatives of the same file), F-HLP-012 (how the program speaks about an unavailable source) |
| Breaks compatibility | no |

## Verdict

We do not set up the server half: there is no participant, and "sync during shooting" without a
second machine is not a feature but its name. The formula is the same as for F-PRJ-003 and F-PRJ-001:
we take the local core now, and the server half when a second person appears.

The local core here is one thing and it is narrow: we have no notion of "a file that is still being
written", and this is visible not in someone else's cloud feature but in our own code — the duration is
asked once, a failure is remembered forever, and "warmth" keeps asserting things about content that
was not yet in the file at the moment of decoding. This is a defect of the class "the program remembers
something stale and does not re-ask", and it costs one request to the worker, not a round.

What must NOT be done: setting up a file system watcher and automatic lengthening of events. The first
gives background work during a gesture and playback (rule 12), the second gives document growth without a
person (rule 20). The right form is a manual command "re-read the source", already existing as
`replace_media`, plus an honest cache reset based on its result.

Boundaries: proxies and their distribution — F-COL-005 (there DERIVATIVES travel, here — the original bytes);
moving the project to another machine — F-PRJ-003; the message about a missing source — F-HLP-012.

## Sources

- Blackmagic Design, DaVinci Resolve — Collaboration:
  https://www.blackmagicdesign.com/products/davinciresolve/collaboration
  (accessed 08.09.2026) — "The ability to transfer media directly into
  the DaVinci Resolve media bin as editors are working is revolutionary and has
  never before been possible!", "As new files are added they will automatically
  sync and appear in the project."
- The live sync mechanism (the camera writes a second file — an HD proxy H.264; list of
  models: Cinema Camera 6K, PYXIS, URSA Cine, URSA Broadcast G2) was assembled from
  announcements and secondary publications (videomaker.com, svgeurope.org,
  provideocoalition.com, tvnewscheck.com, 2026) — verbatim wording of the primary source
  on this point could not be obtained: **[NOT VERIFIED]** as regards the exact text of Blackmagic.
- Adobe Community, «Premiere Growing Files AVF», 17.02.2020 – 22.03.2023:
  https://community.adobe.com/questions-729/premiere-growing-files-avf-1359820
  (08.09.2026) — segmented AVF recording, a five-second duration until the end of
  recording, crashes at the end of a growing MXF, evidence of support for growing files in Final Cut Pro.
- Syncthing docs, «Syncing»: https://docs.syncthing.net/users/syncing.html
  (08.09.2026) — the conflict resolution rule, the name of the losing file,
  "are treated as normal files".
- `forum.blackmagicdesign.com`, thread «Warning: Blackmagic Cloud Not Ready for
  Professional Use» (`viewtopic.php?f=39&t=224635`) — UNAVAILABLE (403,
  Cloudflare); the facts about the stuck queue and the `-conflict` files are taken from search
  results and are marked in the text as secondary.
- The VEGAS, Final Cut and After Effects rows are compiled from vendor documentation and the
  composition of dialogs; no public tracker exists for any of the five
  (`docs/features/_trackers.md`, "Whom we do NOT take") — marked
  **[NOT VERIFIED]** where they were not checked live.
- Our code was read on 08.09.2026: `src/libs/kt-cache/src/lib.rs`,
  `src/libs/kt-media/src/index.rs`, `src/libs/kt-engine/src/player.rs`,
  `src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/bin_panel.rs`.
