---
id: F-COL-015
title: "Read-only" as a standard mode
status: candidate
priority: B
cost: M
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app, kt-model]
invariants: [7]
ui_rules: [16]
round: null
point: null
tests: []
depends_on: [F-COL-013]
trackers: []
services: [team-projects, bm-cloud, premiere-productions]
checked: 2026-09-08
---

# F-COL-015 — "Read-only" as a standard mode

## Essence

The document is occupied by someone else — and this is no reason to show an empty window. An editor who
cannot edit is still obliged to look, scrub the playhead, listen to the sound, capture a frame, export a
piece for showing and carry a fragment away to themselves. If the mode reduces to grey buttons, people do
not use it: they make a copy of the project, and the occupation loses its meaning together with the copy.
"Read only" is a degradation of WRITING, not a degradation of capabilities.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no mode, because there is no occupation: the file is opened by anyone and edited by anyone [NOT VERIFIED live] |
| Adobe Premiere | 26.x | View only (Team Projects, Sequence Locking) | the occupation is on the SEQUENCE; the rest keep a named set: playback, seeking, scrubbing, copying clips and duplicating the sequence — that is, everything that does not write into the occupied sequence |
| DaVinci Resolve | 21 | read only until unlocked | the mode is the NORMAL state of everything you have not occupied: "Bins and timelines are "read only" until unlocked by the current user" |
| Final Cut Pro | 12.3 | none — a refusal instead of a mode | the library is occupied whole, and the second person does not even get a view: "already in use by [user] on [Mac].local", the window does not open at all |
| After Effects | 26.x | View only (Team Projects) | the same Adobe mechanism on compositions; outside Team Projects there is no mode, `.aep` is not occupied [NOT VERIFIED live] |

## The trick behind it

**Allowed is not "looking", but everything that is not a write into the occupied object — and the
boundary is drawn by the purpose of the operation, not by its kind.** Copying clips FROM an occupied
foreign sequence is allowed, although it looks like editing: the clipboard is not a write, it takes the
data to oneself. Duplicating a sequence is allowed for the same reason, and it also serves as the standard
bypass ("make a copy and work"). It is precisely because of this boundary that the mode turns out to be
usable: a person with view-only is not paralyzed, they assemble what they need on their own side and wait
for publication.

**And precisely for this reason a failure reads as a refusal, not as a degradation.** When view-only
gets stuck — and it gets stuck both with a live server and with the only online editor — the person keeps
the SAME declared set of abilities that the vendor considers sufficient. It turns out to be insufficient:
they came to edit. The acceptability of the mode is given not by the list of what can be done in it, but by
the presence of an EXIT from it — publication or removal; a list without an exit turns into a description of
a cell. The conclusion worth taking away: by designing "read only" one designs not a set of permissions but
the condition of an exit.

**The third layer — with us "just look" is physically impossible.** The position of the playhead and the
viewport lie IN THE DOCUMENT (`UiState.cursor` — `src/apps/kt-app/src/app.rs:768`), and the background autosave
treats their change as a reason to write (`moved`, `app.rs:1068`). That is, to open a foreign project, press
space and go drink tea — means to rewrite the foreign file thirty seconds later. The "read only" mode cannot be
obtained with us even by accident: to look is to edit.

## Pros

- The mode removes the main incentive to make copies: while it is possible to look and export, a copy is not needed,
  and therefore the occupation remains meaningful.
- The boundary "clipboard is not a write" gives the second person useful work instead of waiting: they assemble pieces
  on their own side while the owner publishes.
- Viewing and exporting require nothing but reading: no locks, no service files, no coordination — so the mode is cheaper
  than any other element of the topic.
- In a program with one composition code (with us — invariant 7), an export from read-only obviously coincides with what the
  owner would get: the mode does not produce a second quality of the picture.

## Cons

- The set of the mode's abilities is declared by the vendor but not guaranteed: a stuck view-only looks exactly like a normal
  one, and differs only in that it does not end.
- Copying from an occupied document creates a third copy of the truth: a piece carried to oneself may already be wrong by the
  time of publication.
- "Read only" is useless if it is not said at the moment of refusal: a grey button without a reason is indistinguishable from a
  broken program.
- Final Cut has no mode at all, and this shows the price of the choice "occupy the document whole": the larger the unit of occupation,
  the less is left to the second person, down to zero.

## What users say

Adobe itself names the exact composition of the mode — and this is the only formulation on the topic
where what exactly remains is listed:

> playback/seek/scrub, copy and paste clips, and duplicate your sequence
> — Abhi Singh (Sr. Product Manager, Adobe), «Collaborate without conflicts using
> Team Projects in Premiere Pro», Adobe Product Community, 03.10.2022,
> https://community.adobe.com/t5/premiere-pro-beta-discussions/discuss-collaborate-without-conflicts-using-team-projects-in-premiere-pro/m-p/13245499/highlight/true

How the same mode looks when there is no exit from it:

> Team Projects puts me in "view only" mode for all of the sequences.
> — evanf37738972, «Premiere Team Projects giving me "View only" even though I am
> the only editor online», Adobe Product Community, 24.12.2024,
> https://community.adobe.com/t5/team-projects-discussions/premiere-team-projects-giving-me-quot-view-only-quot-even-though-i-am-the-only-editor-online/td-p/15055508

And Blackmagic's wording, from which it is evident that the mode in the industry is the default state, not a punishment:

> Bins and timelines are "read only" until unlocked by the current user.
> — Blackmagic Design, page «DaVinci Resolve — Collaboration» (accessed 08.09.2026),
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration

`trackers:` is empty deliberately (rule 12): the "read only" mode does not occur as a topic in the exported corpus of
twelve open-source trackers — shared work is represented there by one genuine request on 25,442 tasks. A number that
does not exist is more honest than a fabricated filter.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-app/src/app.rs:1991` — `KtApp::apply`: the ONLY funnel through which an edit reaches the model. The gate of the
  mode is one branch here, not a bypass of two hundred handlers. A side detail visible only up close: `model_rev` is incremented
  BEFORE application, that is, a rejected command also moves the revision.
- Playback, scrubbing and the preview do not pass through `apply` at all: the transport lives in the player, and the display is built
  from the frame of the engine — they do not write to the model.
- `src/apps/kt-app/src/render_dialog.rs:193` — `launch_render` CLONES the project and hands the clone to the render thread; the export (of
  both the project and the region) by construction changes nothing in the document.
- The clipboard — `kt_model::ops::ClipBlock` (`src/libs/kt-model/src/ops.rs`): copying a selection assembles a block and does not touch the
  model (t205); pasting — touches it, and this is exactly the boundary the industry draws by the word "write".
- `src/libs/kt-media/src/index.rs` — thumbnails and peaks are read from the sidecar next to the project: one can look at someone else's edit
  without decoding anything (invariant 8).

### What is missing

- The concept itself. Across all of `src/` there are zero matches for `read_only`/`readonly`, and not a single check of file rights: the
  impossibility of writing is discovered at the moment of writing and arrives as an I/O error text — in Ctrl+S as a message, in the background
  autosave as a warning (`app.rs:1093` and the parsing of the response further down the code).
- Honesty on the side of the document. As long as `UiState.cursor` and the viewport are part of the project, "looking" remains an edit, and no gate
  will change this: the autosave will fire on `moved` (`app.rs:1068`) and rewrite the foreign file.
- Messages. UI rule 16 requires showing the impossibility of an action as an error, not as a faded button; today there is nothing to show, because
  there is no prohibition.
- An exit from the mode. The industry teaches that a mode is defined not by the list of what is allowed but by the condition of its ending; with us its
  role will be played by the removal of the occupation (F-COL-013, F-COL-014).

### What gets in the way

What gets in the way is not the gate but the place of residence of the interface state. The cursor, the zoom and the row layout lie in `UiState` inside the
project — this is a deliberate decision (convenience moves together with the edit), and it is the same thing that makes viewing a write. We have already
tried to separate "editing content" from "editing convenience" with the flags `dirty_content` and `dirty_ui`, but the autosave still writes the whole document
when the convenience has diverged. So the price of the mode is not a branch in `apply`, but an answer to the question of where convenience lives when the
document is someone else's.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/apps/kt-app/src/app.rs:1991` — the mode is entirely local and almost assembled: every edit passes through one funnel `apply`, and everything for the sake of which the mode is set up already goes around it — the transport and scrubbing live in the player, the preview is built from the engine's frame, the region export works on a clone of the project, copying a selection assembles a block and does not touch the model; what is missing is not the network but a gate and the honesty of the document, because the position of the playhead lies inside the project, and therefore "looking" with us is written to disk.

NETWORK COST: for "read only" to be ASSIGNED rather than chosen by oneself — access rights — the second person's mode expresses not my caution but someone else's decision: at the industry it is switched on by the fact of a neighbor's occupation, and without a list of people and their roles we will have a voluntary toggle, which anyone who finds it inconvenient can switch off.

NETWORK COST: to trust a frame taken from someone else's document — invariant 7 — an export from the "read only" mode makes sense only when it coincides with what the owner would get, and with us this is guaranteed by one composition code for preview and export, and precisely for this reason "look and hand over a piece" does not require a second half of the mechanism.

### Which invariants it touches

Invariant 7 (one composition code for preview and export) — touched from the useful side: it is what makes an export from
read-only meaningful, because the second person gets the same bytes as the owner, not "an approximate picture for viewing".
Invariant 8 (scrolling and zooming do not generate decodes) — the reason why looking at someone else's edit is cheap: waveforms
and thumbnails are read from the sidecar. From the rules of `src/apps/kt-app/CLAUDE.md` rule 16 is touched: a prohibition must
arrive as a message with a reason, not as a grey button. Rule 2 ("works during playback") gets here a mirror formulation: the
mode has no right to forbid what does not write.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: the gate and the messages; separately — the fate of `UiState` with a foreign document |
| Where the main work is | kt-app (`apply`, autosave, messages), kt-model (the boundary "content / convenience") |
| Project format | the schema is not touched; the RULE of writing the `ui` section of a foreign file IS touched |
| Tests to write | in "read only" mode every command is rejected and the reason is shown; playback, scrubbing, preview, copying and region export work; a project opened read-only is not rewritten by the autosave after an hour of scrubbing (today it fails) |
| Depends on | F-COL-013 (the mode is turned on by the occupation), F-COL-014 (exit from the mode) |
| Breaks compatibility | no |

## Verdict

We take the local half, and it is almost assembled for us for free: edits pass one funnel, and viewing, sound, preview, copying and
region export already go around it — that is, "read only" is separated from us by a gate and a message, not by a subsystem. The value,
moreover, is not only network: the same mode is an honest answer to a file lying on a read-only medium, and to someone else's project
opened "just to look".

The expensive and mandatory part is not the gate, but the fact that viewing today is a WRITE: the position of the playhead and the viewport
lie in the document, and the background autosave rewrites the foreign file thirty seconds after the space bar is pressed. This is fixed on ONE
machine and must be fixed before the mode: otherwise the gate will forbid a person to edit someone else's project while continuing to edit it
itself.

We do not set up the server half by the formula of F-PRJ-003 and F-PRJ-001: an assigned "read only" requires a list of people and their rights,
and there is no second participant. The voluntary mode ("open read-only") is legitimate without it.

Boundaries: occupation and the declaration next to the file — F-COL-013; removal of a lock that outlived its owner — F-COL-014; the owner of a TRACK
as an agreement within one document — F-COL-012; the queue and output parameters — F-EXP-002; unsaved work on exit — F-RES-005; a message about an
impossible action — F-HLP-010.

## Sources

- https://community.adobe.com/t5/premiere-pro-beta-discussions/discuss-collaborate-without-conflicts-using-team-projects-in-premiere-pro/m-p/13245499/highlight/true
  — Adobe Product Community, 03.10.2022 (accessed 2026-09-08): the composition of view-only is listed by an Adobe employee.
- https://community.adobe.com/t5/team-projects-discussions/premiere-team-projects-giving-me-quot-view-only-quot-even-though-i-am-the-only-editor-online/td-p/15055508
  — Adobe Product Community, 24.12.2024 (accessed 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  Blackmagic Design (accessed 2026-09-08).
- https://support.apple.com/en-us/HT207128 — Apple: an occupied library does not open at all, there is no viewing mode (accessed 2026-09-08).
- The VEGAS and After Effects rows — from documentation, not checked live; marked `[NOT VERIFIED]` in the table.
- Our code was read on 08.09.2026: `src/apps/kt-app/src/app.rs` (768, 1068, 1093, 1991), `src/apps/kt-app/src/render_dialog.rs` (193),
  `src/libs/kt-model/src/ops.rs`, `src/libs/kt-media/src/index.rs`; a search for `read_only`/`readonly` across all of `src/` — zero matches.
