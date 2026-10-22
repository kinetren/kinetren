---
id: F-COL-021
title: Two notes on one frame
status: researched
priority: B
cost: S
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: no
crates: [kt-model, kt-app]
invariants: []
ui_rules: [16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-COL-020]
trackers: []
services: [frame-io, syncthing]
checked: 2026-09-08
---

# F-COL-021 — Two notes on one frame

## Essence

Two people watch the same review cut and both stop on the same frame: the director says "the cut is early", the sound engineer says "here is a click". This is neither a coincidence nor a rarity: a freeze frame is a common language, so problem spots coincide among different people more often than they diverge.

The question of this note is one: what does the program do when a second note arrives on an occupied frame? There are exactly three answers — stack it, refuse it, or silently discard it — and the third occurs more often than it seems.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Markers / Regions | a marker is addressed by position; there are no two markers at one point of one ruler. The historical workaround is REGIONS: they overlap, so a "second note" is expressed by a second area, not a second marker `[NOT VERIFIED]` for 2026.0 |
| Adobe Premiere | 26.x | Sequence markers + clip markers | a second "floor" exists, but it is not about people: a marker lives both on the sequence and on the clip, and these are two different spaces of addresses. Frame.io replies arrive in one of them, and when two comments on one frame are imported, the second marker lands on the same spot as the first |
| DaVinci Resolve | 21 | Markers (timeline / clip) + Flags | three carriers at once: a timeline marker, a clip marker and a flag on the source. A flag has no time at all, so "two notes on a frame" are partly solved by changing the CARRIER, not by resolving the duplicate |
| Final Cut Pro | 12.3 | Markers on clips | there is no ruler for markers at all: "You work with markers in the same way for clips in the browser and clips in the timeline". So two twin frames are two clips, and on the shared vertical of one moment there can be as many markers as there are clips under the cursor |
| After Effects | 26.x | Layer markers + Composition markers | exactly two levels, and they are independent: a layer marker and a composition marker can stand on one frame, because they belong to different objects. The number of notes per frame = the number of layers plus one |

**Outside the core.** Two sources are needed: one where the limitation is FORMULATED together with its scope of action, and one where a collision is resolved formally, not by eye.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Live Timecode Notes (editingtools.io) | 2025–2026 | logger session | "Only one comment is allowed per timecode in a single user session" — uniqueness by time is declared explicitly and LIMITED to one session of one person; in a team event there are up to 100 loggers, and each puts their note on the same timecode, because there are as many spaces as there are names |
| OTRN | 1 (November 2025) | `notes[].replies` | a reply to a note is not a second note, but an element of the `replies` array inside the first: "An array of reply objects for threaded discussion. Can be empty". A second statement about a frame is expressed by nesting, not by adjacency |
| Syncthing | 2.x | `.sync-conflict-…` | the reference for formal conflict resolution: the loser does not disappear but is RENAMED to `<name>.sync-conflict-<date>-<time>-<who>.<ext>`; conflict files "are treated as normal files" and spread to all nodes. The rule is deterministic to the last bit: the older modification time loses; on a tie, the device with the larger first 63 bits of the device ID |

## The trick behind it

**1. Uniqueness of a note by time is a legitimate rule for ONE author and a direct ban on a second one.** The wording of Live Timecode Notes shows this verbatim: the restriction "one note per timecode" exists, but its scope is `a single user session`. Within one stream of consciousness two notes on one frame are indeed pointless: the person will just finish the first. Extend the same scope to the timeline and the very same rule becomes "the second has nothing to say", and the code does not change by a single line in the process — only the number of people sharing one address space changes.

**2. Everyone who "can" put two notes on a frame does it not for people but for OBJECTS.** In Premiere the second floor is a clip marker versus a sequence marker; in After Effects, a layer marker versus a composition marker; in Resolve a third is a flag, which has no time at all; in Final Cut there is no ruler, and the number of notes on a frame equals the number of clips under the cursor. That is, the capacity of a frame is nowhere equal to the number of people — it is equal to the number of carriers, and a second reviewer lands in the stack by accident, by occupying a free floor. Precisely for this reason importing a review onto one track collapses the opinions while remaining "formally correct per the specification".

**3. The industry way out of the deadlock is not to allow the duplicate but to change the relation.** OTRN does not add the right for a second note to stand on the same frame: it makes the second note a REPLY inside the first (`replies`). This changes the question: not "how many notes fit on a frame", but "is this one topic". And that is enough for the time uniqueness to stop interfering: on a frame there is always one thread, and there can be as many statements in it as you like.

**4. The industrial answer to a collision is known, and it is not about editing.** Syncthing is the only place where the rule is written in full: the loser is renamed and remains an ordinary file, which spreads across all nodes. What matters is not the naming scheme but the principle: the side resolving the conflict MUST NOT reduce the number of objects. Anything that silently collapses two into one differs from this not by strictness, but by the fact that the loss is never seen.

## Pros

- One note per frame within one author is the right default: it turns an accidental repeated gesture into an edit of what has already been said, rather than into junk.
- Different carriers (clip, track, edit) provide capacity for free and at the same time answer the question "what is this note about" — about the frame or about this take.
- A thread of replies instead of adjacency makes the address by time sufficient: the stack does not need its own order, it has the order of replies.
- A formal rule for resolving a conflict (who loses and what happens to the loser) is more valuable than any heuristic: it can be checked by a test and explained to a person.

## Cons

- Capacity by the number of carriers is an illusion of capacity: two people working in one layer still fight over one cell.
- A stack on a frame is unreadable by eye: flags are drawn at one point of the ruler, and until you hover, only one note is visible.
- A reply thread requires an identity for the root note — that is, the very `noteId` for whose sake OTRN was created; without it the reply has nowhere to attach.
- The rule "the loser is renamed" multiplies objects: in Syncthing conflict files spread across all nodes, and they have to be cleaned up by hand.

## What users say

The most valuable thing here is not a complaint but a place where a restriction is named TOGETHER with its scope of action. This is exactly the line that separates "a rule for one author" from "a ban on a second one":

> Only one comment is allowed per timecode in a single user session.
> — Live Timecode Notes, section "The basics of this tool", editingtools.io,
> https://editingtools.io/livenotes/ (accessed 2026-09-08)

The negative half of the feedback concerns what happens when someone else's review does land on an occupied spot: the Adobe thread about Frame.io V4 is titled "keeps overwriting comments markers", and the workaround there turned out to be closing the panel entirely (Andrew C. Brinsmead, 28.10.2025,
https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768).

## How it fits Kinotrain

### What we already have

We implement the rule "one marker per tick" THREE times, and all three times differently — and that is the finding of this note.

1. **An honest error.** `Command::AddMarker` checks occupancy and returns `CommandError::MarkerExists(Tick)`; `MoveMarker` does the same for the target
   (`src/libs/kt-model/src/model.rs`). This is exactly UI checklist rule 16:
   an impossible action is shown as an error, not as a faded hint.
2. **A counted refusal.** When pasting a batch, colliding markers are skipped, and the skip IS COUNTED and shown as a banner: `skipped.markers` in
   `src/libs/kt-model/src/ops.rs` and the message `PASTE_SKIPPED_MARKERS` in
   `src/apps/kt-app/src/app.rs` (t249, copy-paste R.35).
3. **A silent loss.** `Project::normalize()` on load does `sort_by_key` + `dedup_by_key` by `m.t`
   (`src/libs/kt-model/src/project.rs:3488`). The comment next to it honestly
   explains why this is done — a stable sort is needed for the byte stability of the project (test 12) — but the FACT remains: a foreign file with two markers on one tick opens without a single word, and one of them disappears.

The third path contradicts the first two by UI checklist rule 17 ("paired ways of doing one action behave the same") and by rule 20 ("nothing grows or switches off by itself"). Of three answers to the same question, two talk to the person, and the third does not.

Additionally: a range marker with a non-empty name occupies the COMMON namespace with areas (`Project::taken_names`, `validate` → `ProjectError::DuplicateName`,
`src/libs/kt-model/src/project.rs`). That is, on one frame one can not only not have two flags — one cannot even have two identical words.

OFFLINE: DOES NOT REMAIN — `src/libs/kt-model/src/project.rs:3488` — a second note on a frame does not exist even for one person: sorting and dedup by time sit in the normalisation, which runs on EVERY load, so "the cut is early" and "here is a click" on one tick never coexist even for a second; and of our three answers to a collision (command error, a count of skips on paste, dedup on read) exactly this single one is silent — the loss is visible only in a diff of the file.

NETWORK COST: for a collision to be resolved rather than collapsed, one needs the names of the participants and a record of both versions — user account and edit log. Today there is nothing to compare: a marker has neither an author nor a sequence trace, undo keeps snapshots of the whole project, and "who lost" cannot be computed from anything, so renaming the loser by the Syncthing rule is impossible.

### What is missing

- Capacity of a frame greater than one — in any form: a stack, a reply thread, or a second carrier (a marker ON THE CLIP, see F-COL-023).
- Uniform behaviour of the three paths: loading must count what it discards and say so, as paste does.
- Readability of a stack: two flags at one point of the ruler cannot be drawn today.

### What gets in the way

The deduplication is not an oversight: it keeps the byte stability of the file (test 12). It cannot be removed — we can only stop being SILENT: a skip on loading is counted the same way as on paste, and goes to the notice list on project opening that we already have (R.33, missing sources). This is the cheap and honest half.

The expensive half runs into the decision of R.35 item 4 (`docs/user-feedback.md`): "rejected the option of creating real ids: markers are addressed by time today". While the address is the identity, a second note on a frame has nowhere to get its identity from — and this is not a defect but a conscious cost, which only a new request from the user can revisit.

### Which invariants it touches

None of the twelve. It touches three UI checklist rules: 16 (impossibility is an error, not silence), 17 (paired paths behave the same — today three paths behave in three ways) and 20 (nothing switches off by itself — and `dedup_by_key` switches it off).

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage for the honest half: loading counts the discarded markers and reports it through the same mechanism as paste |
| Where the main work is | kt-model (`normalize` returns a count instead of silent dedup), kt-app (a line in the notice list on project opening) |
| Project format | does not touch: nothing changes on disk, the reading behaviour changes |
| Tests to write | model: a file with two markers on one tick gives count 1 and one notice; parity: the three paths (command, paste, load) answer a collision the same way by CONSEQUENCE — next to t249 and t89 |
| Depends on | F-COL-020 (without text on a reply a stack on a frame is meaningless) |
| Breaks compatibility | no |

## Verdict

Take the cheap half now, do not take the expensive one. The cheap half: remove the third, silent answer to a collision — loading must count what it discards and say so exactly as paste does. This is UI rule 20, and it is not about a second person — it is about the fact that a foreign file must not be opened with silent loss.

The expensive half — a frame's capacity greater than one — we do not take: it requires an identity for a marker beyond its time, and this option was rejected in R.35 item 4 by a direct decision of the user. There is no participant for whose sake it would be worth reopening it.

The difference from F-UI-005: there the subject is colour as DATA of an event and its owner (source or clip); here the subject is how many statements fit into one moment, and colour has nothing to do with it. The difference from F-TL-007 is even sharper: that note refused to create a second entity next to the marker, and we continue that verdict — capacity is sought in the existing marker (a stack, a thread, a carrier), not in a new object.

## Sources

- https://editingtools.io/livenotes/ — Live Timecode Notes: "Only one comment is allowed per timecode in a single user session", up to 100 loggers in a team event (accessed 2026-09-08)
- https://otrn.editingtools.io/ — OTRN v1 (November 2025): the `replies` field, "An array of reply objects for threaded discussion. Can be empty"; `noteId` as the identity of a note (accessed 2026-09-08)
- https://docs.syncthing.net/users/syncing.html — the conflict resolution rule: renaming the loser, "are treated as normal files" (accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768
  — "keeps overwriting comments markers", 28.10.2025 (accessed 2026-09-08)
- https://help.frame.io/en/articles/9105278-comments-panel-overview — comment sorting: by timecode by default, but also Newest/Oldest and Commenter; replies form a thread and do not take part in filtering (accessed 2026-09-08)
- https://support.apple.com/guide/final-cut-pro/intro-to-markers-ver397279dd/mac
  — "You work with markers in the same way for clips in the browser and clips in the timeline" (accessed 2026-09-08)
- The VEGAS and Premiere rows regarding "two markers at one point" were assembled from the composition of interfaces and help pages; not checked live, and none of the five has a public tracker (`docs/features/_trackers.md`, "Who we do NOT take"). The VEGAS row is marked `[NOT VERIFIED]`
- Our code checked by grep over `src/` on 08.09.2026: `kt-model/src/model.rs`
  (`CommandError::MarkerExists`, five marker commands), `kt-model/src/ops.rs`
  (`skipped.markers` on block paste), `kt-model/src/project.rs`
  (`normalize` with `dedup_by_key`, `taken_names`, `validate`),
  `kt-app/src/app.rs` (banner `PASTE_SKIPPED_MARKERS`); `docs/user-feedback.md`,
  R.35 item 4
