---
id: F-COL-027
title: Acceptance status is a state, not text
status: candidate
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-app, kt-otio]
invariants: [12]
ui_rules: [7, 13]
round: null
point: null
tests: []
depends_on: [F-UI-005]
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-027 — Acceptance status is a state, not text

## Essence

Someone watches a piece and issues a decision on it: "under review," "redo," "accepted." The decision refers not to a file and not to a rectangle on the timeline, but to the WORK on the piece: the material has not changed, but the state has; and each value has a consequence — after "redo" the piece must return to work, after "accepted" it is not touched. No editing program has such an object. That is why the decision always moves into the color of a marker, and the meaning of the color stays in the head of whoever set it.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | an event has no marker of its own at all — the event's color equals the track's color (analyzed in F-UI-005). An event has Switches (Mute, Lock, Loop) — states WITHOUT an author and without any relation to acceptance; the decision is recorded in the region's name |
| Adobe Premiere | 26.x | Label (16 labels with editable names); the actual status — only in the Frame.io panel | the status lives NOT in the project but on the asset of the service: three values "Needs Review," "In Progress," "Approved" on the UPLOADED version of the clip; in V4 own statuses were added. Changing the status sends a notification to the project's participants. The sequence knows nothing about the decision |
| DaVinci Resolve | 21 | Flag · Clip Color · Marker; in collaborative mode — shared markers and chat | there is no status: three colored mechanisms are separated by scope (F-UI-005), and none of them describes the state of the work. In analyses of collaborative mode this is named the main gap — there is no way to see what has been accepted and what has not |
| Final Cut Pro | 12.3 | Favorite / Rejected (rating) | **the only real state machine among the five**: three states (favorite → unrated → rejected), set on a RANGE, not on the whole clip; drawn as a green and a red LINE along the top of the frames; the "Hide Rejected" filter makes the state operative, `U` (Unrate) returns it to neutral. But the object is the material, not the work on it |
| After Effects | 26.x | Label; the same Frame.io panel | a layer has a color with an editable name, a piece of work has nothing; the status, as in Premiere, lives on the service's asset |

**Outside the core.** All five core rows are in essence "none," so the mechanism cannot be described by the five: one can only see what it degenerates into. The structure of the status lives where it exists by profession — in pipeline trackers, and both programs below are given for the sake of the structure, not for the sake of a verdict.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kitsu (API Zou) | documentation as of 08.09.2026 | Task Status | the status is a TABLE ROW: `name`, `short_name`, `description` ("Explanation of what this status means"), `color`, `priority` (display order), and seven booleans — `is_done`, `is_wip`, `is_retake`, `is_feedback_request`, `is_default`, **`is_artist_allowed`** ("Artists can set tasks to this status") and **`is_client_allowed`** ("Clients can set tasks to this status"). The color is described as "Hex color for visual identification" — that is, it is openly declared as decoration |
| AYON | help.ayon.app as of 08.09.2026 | Statuses in Project Anatomy | a status has a NAME (arbitrary, renamable) and a separate machine `state`; the server reads the state, not the name: "AYON identifies the first status with an `in_progress` state and uses it as the default status for all new publishes". The order of statuses in the anatomy is the order of their display across the whole server; the scope is declared by entity types, so a version may have its own set of states and a task its own |

## The trick behind it

**1. The right to say "accepted" stands ON A STATE, not on a person.** Each Kitsu status has two separate boolean fields — `is_artist_allowed` and `is_client_allowed` — and their descriptions in the API read verbatim: "Artists can set tasks to this status," "Clients can set tasks to this status." That is, the question "who has the right" is answered by a COLUMN of the status table, not by a role check somewhere in the code: the state machine is stored as data, together with transitions, the initial value (`is_default`) and the meaning of each value (`description`). A colored NLE label does not express this in principle — paint has no place where "who is allowed to paint with it" could live.

**2. The name is for the person, the `state` is for the machine, and this is separated deliberately.** In AYON the server's behavior depends on the machine class of the state and on the ORDER of statuses, but never on the label: renaming changes nothing. In editing programs it is exactly the opposite — the machine reads the color (filters and `Select Label Group`), while the meaning lives in the author's head. Hence the conclusion of F-UI-005 about anonymous color: it is not about the palette, it is about the fact that the meaning ended up in the one place that does not survive a week.

**3. When the set of states is fixed and cannot be renamed, people bend the label.** Frame.io shipped exactly three values, and the request that held out for years as No. 1 was not "add statuses" but "let us rename 'Needs review' to 'Rejected'" — that is, people needed a state the state machine did not have, and they tried to substitute it with the label of an existing one. Custom statuses appeared only in V4.

**4. A detail of someone else's layout, directly useful to us.** Final Cut draws the state as a LINE along the top edge of the frames, not as a fill, and sets it on a RANGE, not on a clip: the state takes not a single pixel from the content and does not conflict with what is already drawn inside. This is a ready answer to rule 13 of the checklist, obtained not by reasoning but from someone else's practice.

## Pros

- An automaton stored as data can be configured without a release: the set of states, the initial value and "who is allowed" are table rows, not branches in code.
- Separating "name for the person / class for the machine" makes renaming safe: behavior does not depend on the label, so the label can be written in the studio's language.
- A state has its own explanation (`description`) — the meaning is stored next to the value, not in correspondence and not in the author's memory.
- A state on a RANGE with a line along the edge (FCP) adds a channel without taking area from the content, and the "hide rejected" filter turns the state from decoration into a query operator — by it one can not only look but also search.
- A change of state is an event with an addressee: Frame.io sends a notification about it, that is, the transition is visible not only to the one who opened the file.

## Cons

- Half of the mechanism is meaningful only with two persons: without them `is_artist_allowed` and `is_client_allowed` are two always-true fields, and "accepted" turns into a note to oneself.
- A status is a property of the work, and work changes. Trackers therefore attach it to the version/publication; in an editor there is nothing to attach it to, and "accepted" silently survives the re-edit of a piece, that is, it lies exactly when it matters most.
- A fixed set imposes someone else's pipeline, a free set slides into the same anonymous palette, just in words. Frame.io went through both stages in a row, and the second cost it four years.
- One more colored channel on the event runs into the occupancy of channels analyzed in F-UI-005: there is no free paint on the clip.
- The notification is half the value of the transition, and this half lies entirely on the service's side: there is nothing to express it with in the project file.

## What users say

What happens when the set of states is fixed (the thread — a request to rename one of three values; people tried to make the status "Needs review" mean "Rejected"):

> +1 on this. very counterintuitive and would even pay extra to customize these
> choices.
> — Steadynowadmin, 03.11.2023,
> https://forum.frame.io/t/change-the-display-name-of-the-status-inside-frame-io-from-needs-review-to-rejected/840

> I can confirm this is one of our #1 feature requests and that we will be
> addressing this in the future.
> — jhodges (Frame.io), 16.11.2023, same thread

> Custom statuses are available in Frame.io V4.
> — jhodges, 03.09.2024, same thread (the request of 27.10.2023 was closed by a major version change of the service)

And from the side of an editor who has no status at all — about what exactly is paid for its absence:

> There's no clean workflow for review and approvals, making it difficult to see
> what changes have and have not been approved.
> — Evercast, "How to collaborate in DaVinci Resolve," 26.02.2026,
> https://www.evercast.us/blog/how-to-collaborate-in-davinci-resolve

The `trackers:` field is empty deliberately: the subject "acceptance state" is not registered in the trackers of editing programs — there is nothing to violate, no defect can exist, and the wish is formulated as "give us color labels" (this is already accounted for in F-UI-005). Putting someone else's bill here would mean counting the same signal twice.

## How it fits Kinotrain

### What already exists

- **The only user-facing paint in the whole program — the area palette:** `AREA_PALETTE` in `src/apps/kt-app/src/theme.rs:43` — five pairs "`#rrggbb` → color," and none of them has a name; the parsing is strict, `parse_hex` (`src/apps/kt-app/src/theme.rs:53`) returns `None` for a foreign value ("draw as usual"). It was introduced by R.38 item 1 — that is, we ALREADY have exactly that mechanism into which all five will migrate the status, and ours is just as anonymous.
- **The rest of the palette is occupied by KINDS, not states:** `LANE_VIDEO`, `LANE_AUDIO`, `LANE_MORPH`, `LANE_CAMERA`, `MORPH_SLOT`, `AREA` (`src/apps/kt-app/src/theme.rs:29-63`). There is no free color for a state — the same conclusion that F-UI-005 reached from the side of the event.
- **The document item closest to an "acceptance object" is the named area** (`MarkedArea`, `src/libs/kt-model/src/project.rs`): a name-address, time, a set of tracks; it goes through undo, travels in the file (schema v4), and the track keys within it are semantic. This is the only entity that addresses a PIECE OF WORK, not a file and not a rectangle.
- **The habit "a state is a TYPE, not a number" already exists with us:** `SlotSel` in `src/apps/kt-app/src/widgets.rs:272` was introduced as a separate type with a direct rationale — otherwise an offset of effect indices by one is guaranteed to give an off-by-one. This is exactly what distinguishes a status from a label.
- **A state label must fit into the occupied place:** name truncation is one thing for all points in the program — `widgets::elided_name` (`src/apps/kt-app/src/widgets.rs:326`, rule 7 of the checklist, t255).
- **The only `locked` in the program — belongs to a bin group:** `KtApp::bin_locked` (`src/apps/kt-app/src/bin_panel.rs:118`) — a state without an author and without a transition, cleared by the same click that sets it. On the card itself (preview 128×72, name, duration) there is no channel for a state at all.

OFFLINE: DEGENERATES — `src/apps/kt-app/src/theme.rs:43` — the automaton remains entirely: a closed set of values, the initial one, "redo" and "done" are meaningful on their own, because the editor marks THEIR OWN work ("come back here," "this piece is handed in"); what degenerates is exactly the column of rights — the two "who is allowed" fields collapse into one person. Yet today a state has nowhere to land except an anonymous color: the only user-facing paint in the program is five `#rrggbb` values without a single name, that is, the very migration of a status into decoration has already happened with us, only without a second participant.

NETWORK COST: the right to say "accepted" — access rights — in someone else's automaton the permission stands ON THE STATE (`is_artist_allowed` and `is_client_allowed` are fields of the status, not properties of a person), therefore without a list of people these two columns cannot be expressed by anything, and acceptance remains a note to oneself, not acceptance.

### What is missing

- **An acceptance object.** The status is a property of the work, and our work is named nowhere: there is no document version in the file (there is only the format version), an edit has no name, and `model_rev` and `saved_rev` do not get into the file. Therefore "accepted" has nowhere to be placed so that it stops being true at the first edit of the piece.
- **A state type.** Neither an area, nor an event, nor media has a field where a state lives; there is only a color — and it, as said above, is anonymous.
- **A query operator.** In Final Cut the state works because next to it there is a "hide rejected" filter. We have no filter either in the bin or on the timeline, and the state would remain purely visual — that is, the same decoration.

### What blocks it

The occupancy of channels and rule 13 of the checklist: a new lane for the status cannot be created, and the whole fill of the event already says everything (source identity, selection, hover, offline, level, label, area, morpher). The shape is therefore suggested by someone else's code: a line along the edge of the range, not a fill.

The second thing, and it is heavier: a status set on a piece must GO STALE when that piece is edited, otherwise it lies. We have no mechanism of the kind "this area has changed since X," because an edit has neither a name nor a base revision in the document: undo is snapshots of the whole `Project`, not operations. So either the state is reset by any edit within the bounds of the area (crude, but honest), or the acceptance object must be immutable — and an immutable object we do not have yet either.

### Which invariants it touches

- **12** (repaint only on motion, no allocations, no clones in draw paths): the state must land in the same cache keyed by `model_rev` as the labels, and be drawn as one line, not as a set of shapes per frame.
- **Rule 13** of the UI checklist — there will be no new lane for the state; **rule 7** — the state label is truncated via `elided_name` and does not break the layout.
- Format: the state field of an area lies in our `kt` section, an old reader will read the area without it — therefore a content bump, as with `lane_order` in R.35, rather than a hard refusal to read.

### How it differs from neighboring notes

- **F-UI-005** (clip color as data and as decoration) fixes the anonymity: a label gets a color PLUS a name. But a named label "Accepted" is still set by anyone, at any moment, to any value, and removed with one click — exactly like the track lock in F-COL-012. A status is not a value but an AUTOMATON: a closed set, an initial value, allowed transitions and a consequence for each. And the subject differs: a label describes an OBJECT ("this is an interview"), a status describes the WORK on it.
- **F-MED-001** (smart folders and search) — properties of MATERIAL and selection by them. The acceptance status is not a property of the file: the same file in one edit is accepted, and in another it is sent for redo, and it changes without any change of bytes. It cannot physically be put into the media index — `.ktindex.db` is keyed by the absolute path and the file identity (size and mtime), and a status is bound to neither.
- **F-COL-012** (track owner) — about membership of a row; here — about the state of a piece of work. The intersection is exactly one thing: here and there, without a list of people the second half of the mechanism degenerates, and both notes refuse to introduce it.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: a state type on an area with transitions and reset on edit, then display as a line and a filter |
| Where the main work is | kt-model (a closed set of values, the initial one, a transition command via undo, invalidation on edit within the area's bounds), kt-app (a line along the range edge, a "hide accepted" filter, a row of verbs in the t253 registry), kt-otio (a field in the `kt` section and reading of old files) |
| Project format | a content bump (as `lane_order` in R.35): an old build will read the area and lose the state |
| Tests that will have to be written | a state round-trip through the model, undo and the file; "an edit within the area's bounds resets the state"; "a transition that is not in the automaton is a command error, not a silent one"; a layout test "the state line took not a single pixel from the content"; a row in the verb table t253 |
| Depends on | F-UI-005 (there it is decided what colors the event; here — what the state means) |
| Breaks compatibility | no |

## Verdict

We take the local core — **the state as a TYPE on a named area**, with a closed set of values, an initial value, an explanation for each value, and a reset on an edit of the piece. This works on its own and is needed on its own: marking one's own work "redo / handed in" and hiding the handed-in items is the same value as the Final Cut rating, only about the work, not about the material. We draw it as a line along the edge of the range: a channel is added, and no pixels are taken from the content.

We do not take the second half — **roles and rights**. This note has no right to declare a second participant, and "who is allowed to set this state" without a list of people is two always-true fields. The formula is the same as for F-PRJ-001 and F-PRJ-003: the local core now, the server half is not introduced.

And separately, so that a year from now nobody argues about the wrong thing: **color is not suitable for a status in principle**, even a named one. The difference is not that there are few colors, but that paint has neither a closed set, nor a transition, nor a "who is allowed" column — and a status is exactly those. Our own `AREA_PALETTE` of five anonymous `#rrggbb` values shows where the state will end up if fields are not created for it.

## Sources

- Frame.io Developer Forum, thread "Change the display name of the status inside frame.io from Needs review to Rejected" (opened 27.10.2023, replies 03.11.2023, 16.11.2023, 01.08.2024, 03.09.2024, 17.10.2024):
  https://forum.frame.io/t/change-the-display-name-of-the-status-inside-frame-io-from-needs-review-to-rejected/840
  (access date 2026-09-08)
- Frame.io V4 Knowledge Center, "Player page features" — three status values in the Properties panel and a notification to participants on change:
  https://help.frame.io/en/articles/9105311-player-page-features (2026-09-08)
- Kitsu / Zou API, object Task Status — the set of fields and verbatim descriptions of `is_artist_allowed`, `is_client_allowed`, `color`:
  https://mintlify.wiki/cgwire/zou/api/tasks/task-status (2026-09-08).
  Kitsu user documentation on statuses:
  https://kitsu.cg-wire.com/status/ (2026-09-08)
- AYON, "Project Anatomy" — statuses, their order and the machine `state`:
  https://help.ayon.app/articles/3815114-project-anatomy (2026-09-08)
- Apple, Final Cut Pro — "Rate clips": a green and a red line along the top of the frames, rating of a range, the "Hide Rejected" filter, `U` = Unrate:
  https://support.apple.com/guide/final-cut-pro/rate-clips-ver30ccd91f/mac
  (2026-09-08)
- Evercast, "How to collaborate in DaVinci Resolve," 26.02.2026 — on the fact that there is no acceptance in Resolve's collaborative mode:
  https://www.evercast.us/blog/how-to-collaborate-in-davinci-resolve (2026-09-08)
- The rows for VEGAS, Premiere, Resolve and After Effects, as far as colored mechanisms are concerned, are taken from the already verified reconnaissance of F-UI-005
  (`docs/features/ui/viewer/clip-colors-and-legibility.md`), so as not to recount the same fact twice; `forum.blackmagicdesign.com` for
  Resolve still returns 403, so there is no verbatim quote from it
- The unit of occupancy for services (the asset for Frame.io, the bin and timeline for Blackmagic Cloud) — `docs/features/_services.md`, column "Unit of occupancy"
- Our code checked by reading on 08.09.2026: `src/apps/kt-app/src/theme.rs`
  (`AREA_PALETTE`, `parse_hex`, the kind palette), `src/apps/kt-app/src/widgets.rs`
  (`SlotSel`, `elided_name`), `src/apps/kt-app/src/bin_panel.rs`
  (`bin_locked`, composition of the card), `src/libs/kt-model/src/project.rs`
  (`MarkedArea`)
