---
id: F-COL-019
title: Who is in the project now
status: researched
priority: C
cost: M
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136]
we_have: partial
crates: [kt-model, kt-app, kt-engine]
invariants: [12]
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [team-projects, bm-cloud, frame-io]
checked: 2026-09-08
---

# F-COL-019 — Who is in the project now

## Essence

The second person is either in the document right now or is not, and every decision of the
editor depends on this: whether this sequence can be touched, whether to write in the chat,
why a piece has suddenly become read-only. The program knows the answer — it holds the lock
precisely on this sign — but says it rarely and at the most inconvenient place: at the moment
of refusal. A separate, far more ambitious question — where exactly the second one now stands
and what they see — nobody in the editing editors solves.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no shared work, presence is expressed by nothing: in the project there is always one, and the program does not reason about it |
| Adobe Premiere | 26.x | the "Project Locked" column (Productions), the "Changes" column and a caption in the top right corner of the timeline (Team Projects) | presence is a DERIVATIVE of the lock: it is not a person that is shown but the name of the one who occupied the object. There is no foreign cursor; to learn "who exactly is there" you are offered to look at the list column or at the caption by the timeline |
| DaVinci Resolve | 21 | read-only for occupied bins and timelines + built-in chat | the same principle plus the industry's only channel of direct conversation inside the program: "who is here" is answered by the object being occupied, and "what they are doing" — by text which the person writes themselves |
| Final Cut Pro | 12.3 | a message when trying to open the library | presence is seen EXACTLY ONCE and only as a refusal: "already in use by [user] on [Mac].local". Until you try to open it, you do not know |
| After Effects | 26.x | has none of its own; through Team Projects — the same list and the same caption | a file project has no presence at all; in the service AE inherits someone else's surface entirely |

**Outside the core.** Without a text editor the trick cannot be shown: only next to it
does it become visible that what hinders editing people is not complexity but the subject.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 (Live Share) | co-editing, participants list, follow | a foreign cursor and selection are drawn right in the document with a name flag, participants are listed, and "follow" synchronizes your editor with the foreign file and scroll position. It works because the position in the text IS the content: by showing the cursor you have shown everything |

## The trick behind it

**No editing program draws a foreign playhead, although office and text editors have drawn
a foreign cursor for twenty years — and the reason is not complexity but that what would have
to be shown is a FRAME, not a position.** In text a position is self-sufficient: line 42 is
visible to both, and a foreign cursor in it is immediately meaningful. In editing, "he is at
00:04:12" means almost nothing: the second one has their own preview resolution share, their own
set of enabled effects, their own backend and, if they are in another sequence, altogether different
material. For a foreign position to become a message, one must transmit the picture from their
machine — and that is no longer presence but video communication. Therefore the industry stopped
one step earlier and expresses presence in TEXT: a column in a list, a caption in the corner, a chat.

**Second layer: presence is nowhere measured separately — it is computed from the LOCK.** "Who is
here" is answered by "who occupied", and therefore the quality of the answer is exactly the quality
of the lock: when the lock errs, presence lies. Hence the regular class of complaints "I was given only
read access although I am alone", and hence the support answer that itself names the entire available
surface: look at the list column or at the caption by the timeline, there it is written who the program
CONSIDERS to be working. From presence as a separate fact — who opened, when, whether still alive — nothing
has been made; there is only the shadow of the lock.

**Third layer: the only form of presence that has taken root in editing is asynchronous.** Resolve's chat
and Frame.io comments work because they do not require simultaneity: a message is addressed not to the
cursor position but to the frame and the person, and is read at any time. Live presence in editing has
remained unsolved — it has been bypassed.

## Pros

- Presence derived from the lock costs nothing: since the lock already exists, "who is here" is
  its very field, shown as a column. No separate channel is needed.
- A text surface (column, caption, chat) does not require simultaneity and survives a broken connection:
  a message does not go stale the way a coordinate goes stale.
- A chat inside the program solves the main thing for which people want to see a foreign cursor:
  to agree on who is doing what — and does it in words that remain.
- Refusal as a form of presence ("already in use by …") at least names the person and the machine: it is
  the poorest surface, but it is unambiguous.

## Cons

- Presence lies exactly where the lock lies, and the person is left without an explanation:
  "read only although I am alone" is a refusal that has no second half.
- Presence is seen late: at Final Cut — at the moment one has already tried to open; at Productions — in a
  column one has to guess to look at.
- From presence one cannot understand INTENT: "occupied by Anya" does not answer whether Anya has gone to lunch
  with the project open. The sign has no liveness.
- A foreign position is not expressed at all, and together with it the most frequent desire of shared work is
  not expressed — "show me what you are looking at"; it is replaced by screen sharing, that is, a tool outside
  the editor.

## What users say

Presence as the shadow of the lock — and what happens when the shadow falls wrongly:

> It seems like Premiere is booting me from editing the sequence because I am editing
> it.
> — evanf37738972, «Premiere Team Projects giving me "View only" even though I am
> the only editor online», 24.12.2024,
> https://community.adobe.com/questions-79/premiere-team-projects-giving-me-view-only-even-though-i-am-the-only-editor-online-1498367

Adobe's answer in the same thread lists the entire surface of presence in full — the column
and the caption in the corner:

> Does it mention which collaborator Premiere thinks is working on the sequence?
> Either in the "Changes" column in the Team Project panel (list view) or at the
> top-right when opening the a sequence in the timeline.
> — Udo_Pawlik (Adobe Employee), 24.12.2024, the same thread

The only direct "person to person" channel inside an editing program is text:

> DaVinci Resolve features built in chat so team members can talk about shots, review
> tasks and share creative ideas without leaving the software.
> — Blackmagic Design, «DaVinci Resolve — Collaboration»,
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration
> (accessed 2026-09-08)

For comparison — how it looks where the position is the content itself:

> Participants can see cursors and edits. They can also see selections you make.
> — Microsoft Learn, «Co-edit in Visual Studio Code — Live Share», 22.01.2022,
> https://learn.microsoft.com/en-us/visualstudio/liveshare/use/coedit-follow-focus-visual-studio-code

## How it fits Kinotrain

### What already exists

- The carrier of "where the other one was" exists with us and travels in the project file: `UiState`
  (`src/libs/kt-model/src/project.rs:1571`) stores `cursor`, `view_start`, `ticks_per_px`, the bin layout,
  the meter width and the track names.
- It is put into the document outside undo — `set_ui_state` (`src/libs/kt-model/src/model.rs:446`), next to
  the region and loop — and reaches the disk along with the nearest save of content (t96) or on exit
  (`src/apps/kt-app/src/app.rs:5308`).
- And it is APPLIED on opening, silently and entirely: `src/apps/kt-app/src/app.rs:707` sets the zoom, the left
  edge of the view and does `player.seek(Tick(ui.cursor))`. Having opened a project saved by someone else's hand,
  I get the other person's playhead as my own.
- The display of position with us is already decoupled from the quanta of the master clock: `video_playhead`
  (`src/libs/kt-engine/src/player.rs:493`) interpolates the position of the DISPLAY between the steps of the audio
  callback by the local `Instant::now()` (invariant 12, test 56).
- Neither a person's name, nor an account, nor the time of the last opening exists in the document: `UiState` has
  no author, and a second such record simply overwrites the first.

OFFLINE: DEGENERATES — `src/libs/kt-model/src/model.rs:446` — presence degenerates into a record about ONE participant, living inside the document: the cursor, the viewport and the zoom are put into the project outside undo and are applied silently on opening. "Where the other one was" is, thus, already implemented with us in the strongest form — it is not shown next to mine, it replaces mine, as soon as a file saved by someone else's hand is opened.

NETWORK COST: a foreign position that MOVES — shared clock · invariant 12 — the display position is derived from OUR audio callback and is interpolated by local clocks, while a foreign playhead has no source of motion on my machine: without common time a transmitted coordinate ages by exactly as much as it has travelled.

NETWORK COST: to say WHO exactly is in the project — an account — a viewport record has no author, and our identifiers are local ("maximum + 1"), so there is nowhere to keep a second viewport and nothing on which to hang it: it overwrites the first.

### What is missing

- Not a foreign playhead — nobody shows it, and we do not need it — but honesty toward our own record: today a
  foreign cursor is applied without asking and without a trace, and the person does not know why the project
  opened not at the beginning.
- A sign of "this file is open by someone else" — the poorest form of presence, the one that Final Cut has. We
  have neither a lock, nor a read of mtime, nor a check for a second instance.
- A place where a person could leave a message to a FRAME, not to a position: a marker has a time, a duration,
  a name and a color, and has neither an author nor text (`src/libs/kt-model/src/project.rs:1459`).

### What gets in the way

Showing a foreign position runs not into transmitting a number, but into the fact that a number without a picture
is meaningless, and the picture at the other end is its own: the share of preview resolution enters the cache key
(invariant 4), the backend may be different, and exactly one MediaWorker owns the decoder (invariant 5) — a foreign
frame cannot be obtained in our architecture except by decoding it on our side, that is, by recalculating it. The
second obstacle is the clock: everything that moves with us is subordinate to the audio callback (invariants 6 and 12),
and a transmitted coordinate has neither a beginning nor a motion in our time.

### Which invariants it touches

Invariant 12 (the display position is interpolated between the steps of the master clock; the raw
`transport.playhead()` is not read during playback) — directly: it describes where the motion of our playhead
comes from, and explains why the foreign one will not have it. Indirectly — invariant 4 (the share of resolution in
the cache key) and invariant 5 (the sole owner of the decoder): they make a "foreign frame" inexpressible other than by
recalculation on one's own side.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages, and both local: ask about a foreign viewport on opening, notice a second writer of the file |
| Where the main work is | kt-app (project opening, the message surface), kt-model (authorship of the viewport record, if it ever becomes data at all) |
| Project format | not touched: `ui` in the envelope already exists, no new fields are needed |
| Tests to write | "opening a foreign file does not move my cursor silently"; a layout test of the line about a second writer |
| Depends on | F-COL-017 (the place of the busy sign), F-UI-005 (the marker-data on an event — a neighbor on "a message to the frame") |
| Breaks compatibility | no |

## Verdict

We do not set up presence — there is no second participant, and a live foreign playhead is set up by no one even with
a participant: in editing a position without a frame reports nothing, and this is not our poverty but a property of the
subject. From the reconnaissance we take two observations, both applicable alone. First: **we already have presence in its
strongest and most impolite form** — a foreign cursor and viewport saved in the document are applied silently on opening,
that is, the foreign position is not shown next to mine, it becomes mine. One should either ask, or not move the cursor when a
foreign file is opened. Second: the poorest surface of presence — "this file has already been touched by someone" — requires
neither a server nor accounts, and it is needed by us for an entirely different reason too (F-COL-018: undo after an external
write silently erases it).

Separately we record as a decision: **we do not show or transmit a foreign frame.** Any future request "show what the second
one sees" breaks against invariants 4 and 5 — the frame would have to be computed on our side, which means it will NOT be their frame.

How we differ from neighbors. **F-COL-014** (a lock that outlived its owner) is about what to do when the busy sign lies; here it is
about the fact that this same sign is the ONLY source of information about people, and therefore its lie has nothing to object with.
**F-COL-013** (two processes over one file) is about declaring occupation; presence lives on the same surface but answers a different
question — not "is it free" but "who and is he alive". **F-COL-012** (the owner of a track) is about belonging that outlives the lifting of
the lock; here it is about the fact of presence here and now. **F-COL-016** is about what is occupied; presence in this pair is derived.
**F-UI-005** (color of a clip as data) and marker-data on an event are about a message tied to a FRAME; this is the neighboring half of the same
conversation, but it is asynchronous and lives in `review/`. **F-UI-003** (window layouts) is about the same `UiState` field, but from the point of view
of one person's convenience, not of the trace of a second one.

## Sources

- https://community.adobe.com/questions-79/premiere-team-projects-giving-me-view-only-even-though-i-am-the-only-editor-online-1498367
  — "view only" although there is one person in the project, and the enumeration of the entire surface of presence
  (24.12.2024, accessed 2026-09-08)
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — the built-in chat and read-only for
  occupied bins and timelines (accessed 2026-09-08)
- https://learn.microsoft.com/en-us/visualstudio/liveshare/use/coedit-follow-focus-visual-studio-code
  — foreign cursors, selections, a name flag and the "follow" mode (22.01.2022, accessed 2026-09-08)
- https://creativecow.net/forums/thread/library-is-already-in-use/ — presence as the only message at the moment
  of refusal (18.10.2017, accessed 2026-09-08)
- The VEGAS and After Effects rows reflect the absence of a mechanism; there is no public tracker for any of the five
  (`docs/features/_trackers.md`, "Whom we do NOT take").
- Our code was checked by grep over `src/` on 08.09.2026: `kt-model/src/project.rs`
  (`UiState`, `Marker`), `kt-model/src/model.rs` (`set_ui_state`),
  `kt-app/src/app.rs` (restoring the viewport and `player.seek` on opening, saving
  on exit), `kt-engine/src/player.rs` (`video_playhead`).
