---
id: F-COL-022
title: Author and time of a comment
status: researched
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: [7, 13]
round: null
point: null
tests: []
depends_on: [F-COL-020]
trackers: []
services: [frame-io, team-projects]
checked: 2026-09-08
---

# F-COL-022 — Author and time of a comment

## Essence

"Re-cut this" without a signature is not a remark but a riddle. The editor needs to know two things: who said it (the director — we do it, the intern — we discuss) and when (before the cut redone the day before yesterday, or after it). Both quantities are present in the review service for EVERY comment and are mandatory by construction: sorting, filtering and counting unread items are done by them.

This note is about what happens to these two fields at the boundary between the review service and the editing program.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Marker / Region name; Command marker Comment | a marker has one name; a separate "Comment" field exists only on a command marker. Neither an author nor a creation time is stored on a marker — the signature is written into the name by hand `[NOT VERIFIED]` for 2026.0 |
| Adobe Premiere | 26.x | Marker: Name + Comments + Duration + Type | TWO text fields on a marker, and the review uses the second: Frame.io carries the contents of Comments, not Name. There is no author or time in the marker's schema — on import they are packed into the comment text |
| DaVinci Resolve | 21 | Marker: Name + Notes; Presentations | a marker also has two fields (name and notes); in Presentations comments and markers are "bidirectionally relinked", that is, authorship lives in the cloud half, and only a link arrives in the project |
| Final Cut Pro | 12.3 | Marker with a comment field; To Do | a marker has text and a state (to-do / done) — the state exists, authorship does not. Frame.io replies arrive in Final Cut not as data at all, but as a "flattened file", that is, as a picture |
| After Effects | 26.x | Layer/Comp marker: Comment, Chapter, URL, cue point | the richest marker schema of the five (several named fields plus key-value pairs of a cue point) — and it still has neither an author nor a creation time. Frame.io comments are visible through the Comments column, where a `file ID` and a link string lie |

**Outside the core.** A source is needed where the fields "who" and "when" are not inferred from the behaviour of the panel but DECLARED by the schema — otherwise the boundary between "the service can do it" and "the format carries it" is not visible.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OTRN | 1 (November 2025) | `commenter`, `timestamp`, `noteId` | a note has "the name of the commenter/user who created the note" and an "ISO 8601 timestamp for when the note was created or updated". Both fields are optional — that is, even a format designed specifically for exchanging remarks does not make authorship mandatory: it knows the source often cannot provide it |

## The trick behind it

**1. A reply always has an author and a time, a marker on the edit never does, and the bridge between them is a single one: TEXT.** The Frame.io help describes the import into Premiere literally: "After importing the comments, the comment, collaborator, and comment time are all in this marker". Three fields fit into one marker — not into its three fields, but into its text. The consequence that appears in no list of capabilities: after import the "author" ceases to be a quantity. It cannot be filtered by, cannot be sorted by, cannot be counted — how many remarks the director made — because they have become part of a sentence. The loss happens not at an error but at a routine success.

**2. The marker has to invent an identity for itself, and it does so in the NAME field.** When Premiere exports markers for review, it appends a service code to the title of each marker: "Premiere Pro always adds something like hex-code to the title of every marker if they are exported TO (not from!) the frame.io for review" — and a second user describes the same picture as "the marker name gets replaced with a code". This is not a cosmetic bug: synchronisation needs a key, the marker has no key, and so it takes the only field the person sees. It is exactly the same disease that OTRN cures with a separate `noteId`.

**3. A marker has two text fields not out of generosity, but because one of them is a service field.** The advice that users arrived at on their own: "Whatever you put in the Comments box shows up as the comment in frame" — that is, notes must be written into the SECOND field, and the first is left empty, otherwise the remark either does not travel or arrives with a code instead of text. A program that has only one field cannot follow this advice in principle.

**4. In After Effects the comment is not stored in the document at all.** "There's a "file ID" and a string of text that connects to Frame.io and it magically pulls the comments in" — the Comments column holds not the text of the review but a reference to it. That is, the richest object of the five by fields still could not accept someone else's statement into itself and accepted its address instead.

## Pros

- Author and time are the only two fields that make a list of remarks workable: without them one cannot set priorities or understand what is outdated.
- Sorting by author (in Frame.io this is a separate mode on a par with timecode) gives the editor a view of "what this particular person is asking for" — in one list.
- The edit time gives a natural answer to "is this note about a previous version" without versions at all: a remark older than the last re-cut is suspicious.
- A state (to-do / done in Final Cut, `complete` in OTRN) closes a remark without deleting it, so the history of the review stays.

## Cons

- The marker schema in all five is designed for a single author, so importing a review ALWAYS flattens the three fields into one. This is not an implementation defect but a consequence of two schemas, and it is reproduced by every vendor.
- A service code in the name field spoils the human name of the marker — the very one that is visible on the timeline.
- Authorship that has got into the text cannot be cleaned out again: parsing a string "Name, time: phrase" is a heuristic that breaks on the first comma.
- A format designed specifically for exchange leaves `commenter` and `timestamp` optional — so even through it authorship does not always arrive.

## What users say

About what a marker pays for not having its own key:

> One thing which IS a bug - Premiere Pro always adds something like hex-code to
> the title of every marker if they are exported TO (not from!) the frame.io for
> review.
> — AndrewTheGreat, 26.08.2024,
> https://community.adobe.com/t5/premiere-pro-discussions/timeline-markers-are-not-getting-exported-as-comments-on-frame-io/td-p/13736692

And the working workaround found in the same thread: write the remark into the second field of the marker and leave the first empty — "Whatever you put in the Comments box shows up as the comment in frame" (Bobby Boson, 13.01.2024, the same thread).

## How it fits Kinotrain

### What we already have

We have neither an author nor a time on a marker — and this is visible in one line: `Marker` is `t`, `duration`, `name`, `color`
(`src/libs/kt-model/src/project.rs:1460`). It has no other fields at all, and `Deserialize` accepts exactly three historical forms (a bare number, `{t, name}`, `{t, duration, name, color}`).

More important is something else: our single text field is already occupied by THREE jobs at once, and "adding a signature to it" is not a workaround but a fourth job on the same key.

1. **The caption on the timeline.** The marker's name is written on the time-caption line, not in its own strip (item 3 of R.12, UI checklist rule 13).
2. **The address.** The name of a range marker lives in ONE namespace together with `MarkedArea`: `Project::taken_names()`, the error `ProjectError::DuplicateName` on loading, and `CommandError::NameTaken` for `SetMarkerName`
   (`src/libs/kt-model/src/project.rs`, `src/libs/kt-model/src/model.rs`).
   Morphers reference an area BY NAME, and renaming corrects the references (R.35 item 4). That is, two reviewers who write the same phrase will produce a file that fails `validate`.
3. **The output file name.** `default_render_name`
   (`src/apps/kt-app/src/render_dialog.rs`) takes the name of the range marker coinciding with the region and substitutes it into the template
   `kt-<project>-<region>-<len>s.mp4` (t201). A reviewer's phrase put into a marker's name will reach the name of the file handed to the client.

There is also a ready place that stands empty today: the OTIO schema `Marker.2` carries the fields `comment`, `metadata` and `extra` (`src/libs/kt-otio/src/schema.rs`), but the reader discards them and the writer writes them empty
(`src/libs/kt-otio/src/convert.rs`). A foreign `.otio` with signed remarks loses them on the first save — although an UNKNOWN marker schema goes into passthrough and survives the cycle whole. Passthrough preserves the foreign better than our own reader preserves the native.

OFFLINE: DOES NOT REMAIN — `src/libs/kt-model/src/project.rs:1460` — a marker has four fields, and among them there is neither an author nor a creation time: today "who and when" the editor holds in their head or writes into the name, and the name for us is simultaneously a caption on the ruler, a unique address in the common namespace with areas, and the template for the delivered file's name, so a reviewer's signature in it breaks both the project check and the render name; a separate text field where it could be placed without consequence does not exist on a marker at all.

NETWORK COST: every reply must have a presentable author and a comparable time mark — user account and shared clock. "Who said it" exists only where participants are distinguishable by the program rather than by the person's memory, and "when they said it" orders the thread only with comparable clocks: two local machines give two incomparable "yesterdays", and there is nothing to restore the order of the remarks from.

### What is missing

- A second text field on a marker — the one that is NOT the address: into it goes both one's own note, and a foreign reply, and a signature, if it is decided to store it as text.
- The time of creation and editing of a marker. This is the only half that is meaningful even alone: "this marker is older than the last re-cut" is a working question.
- Return of the `comment` field of schema `Marker.2` on a round trip — today this is a pure loss of foreign data, not a missing feature.

### What gets in the way

An author without a second participant is a quantity with one value: we have no user accounts and no user name anywhere in the code, and "author = me" carries no information. Therefore the honest boundary runs along time: the time of a reply is measurable on one machine, authorship is not.

The second obstacle is the place in the interface. The text of a remark is an order of magnitude longer than a marker's name, and UI checklist rule 13 forbids creating a strip for the sake of a caption that appears rarely; rule 7 requires measuring the gallery and truncating via `widgets::elided_name`, rather than hoping that "usually it is short". So the text is shown where there is already room (the status line on hover, the list of markers), not in a new timeline row.

### Which invariants it touches

None of the twelve. It touches UI checklist rules 7 (long text is truncated and does not stretch the time-caption strip) and 13 (a second text field must look for an already occupied place). Indirectly it touches t201 (the render file name): as soon as a marker has a second field, the name stops being the only candidate to get into the file name — and this is GOOD, but it requires an explicit decision on which field goes there.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: a text field and a time mark in the model and the format; display and editing in the UI |
| Where the main work is | kt-model (second text field and `created`/`edited` on a marker), kt-otio (`comment` stops being lost, time travels in the kt envelope), kt-app (editing the text and display on hover) |
| Project format | a version is needed: the fields are additive, but an old reader would silently lose them — the R.29 rule |
| Tests to write | `.otio` round trip: a foreign marker with `comment` comes back verbatim (extension of t144/t148); model: the marker's name remains the address, the text does NOT enter the namespace (extension of t89); layout: long text does not grow the caption row (next to t170); t201 stays green — the name of the file goes into the file name, not the text |
| Depends on | F-COL-020 (a reply without an address is not needed, an address without text is useless) |
| Breaks compatibility | no |

## Verdict

Take the half of "when" and the text; do not create the half of "who" — there is no participant, and an author field with a single possible value would be a lie in the schema.

What we take: a second text field on a marker (the one that is not an address), and the return of the `comment` of schema `Marker.2` on the round trip — the latter is not a feature at all, but a repair of a silent loss of foreign data. A time mark for marker creation is useful even to one person.

What we do NOT take: authorship as a model field, and a service key in the marker's name. The second especially: Premiere's path of appending a code to the name field is impossible for us without breaking things — the name of a range marker is an address (morphers reference by name, R.35 item 4) and the template for the render file name (t201). This is the case where a foreign workaround is rejected mechanically by our code, not by taste.

The difference from F-UI-005: there a marker is data of an event — a COLOUR and its owner, a quantity without text; here the subject is a statement and its two service quantities, and what the notes have in common is only the conclusion that an anonymous quantity is useless. The difference from F-TL-007: we do not create a second entity — "who and when" are hung on the existing marker as fields, not as a new object next to it.

## Sources

- https://support.frame.io/en/articles/255191-import-comments-into-premiere-from-the-frame-io-review-panel-legacy
  — "After importing the comments, the comment, collaborator, and comment time are all in this marker" (accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/timeline-markers-are-not-getting-exported-as-comments-on-frame-io/td-p/13736692
  — the hex code in the marker title (AndrewTheGreat, 26.08.2024) and the advice to write into the Comments field (Bobby Boson, 13.01.2024) (accessed 2026-09-08)
- https://community.adobe.com/t5/after-effects-discussions/imported-frame-io-comments-not-visible/td-p/14403508
  — the Comments column with a `file ID` and a link string instead of the text of the review (accessed 2026-09-08)
- https://help.frame.io/en/articles/9105278-comments-panel-overview — sorting by Commenter and by date on a par with timecode; a card shows the author and the time (accessed 2026-09-08)
- https://otrn.editingtools.io/ — OTRN v1 (November 2025): `commenter`, `timestamp` (ISO 8601), `noteId`, `complete` (accessed 2026-09-08). No key was added to `_editors.md`: this is a format, not an editor
- https://support.frame.io/en/articles/2444494-take-the-fcp-tour-legacy —
  replies arrive in Final Cut as a "flattened file" (accessed 2026-09-08)
- The VEGAS and After Effects rows regarding the composition of marker fields were assembled from help pages and the composition of the interface; not checked live, and none of the five has a public tracker. The VEGAS row is marked `[NOT VERIFIED]`
- Our code checked by grep over `src/` on 08.09.2026: `kt-model/src/project.rs`
  (`Marker`, `taken_names`, `validate`), `kt-model/src/model.rs`
  (`SetMarkerName`, `CommandError::NameTaken`), `kt-otio/src/schema.rs`
  (`Marker` with `comment`/`metadata`/`extra`), `kt-otio/src/convert.rs` (both ends of the cycle), `kt-app/src/render_dialog.rs` (`default_render_name`);
  `docs/user-feedback.md`, R.35 item 4
