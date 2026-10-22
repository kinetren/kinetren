---
id: F-COL-020
title: A comment anchored to a frame
status: researched
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: [13]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-020 — A comment anchored to a frame

## Essence

The reviewer watches the video and says "here." The program must turn this "here" into an address that will reach the editor and point there to the very same frame. A remark is not text but a pair "text plus address," and the value is carried by the second half: without an address, a remark turns into an email that the editor sorts onto the timeline by hand.

The whole difficulty is that the addressed thing moves underfoot: the edit is being corrected on the very same evening that the review is taking place.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Markers / Regions / Command Markers | there is no review service of its own: a remark arrives as a list of timecodes and is typed in by hand. The nearest carrier is a named marker and a region; a separate "Comment" field exists only on a command marker, and the help describes it as "a reminder of what the command does," not as a remark |
| Adobe Premiere | 26.x | Frame.io V4 Comments panel | remarks live as cards in the panel; a click on a card moves the playhead; enabled Comment Markers place marks directly on the timeline. The address of a remark belongs to the UPLOADED version of the video, not to the sequence; "annotations, anchored comments, comment attachments, and downloads are not compatible with the Frame.io panel" |
| DaVinci Resolve | 21 | Blackmagic Cloud Presentations | the only one that promises two-way operation: "Shared markers and comments are automatically and bidirectionally relinked with DaVinci Resolve." That is, a remark arrives not as a copy but as a link, and an edit on one end is visible on the other |
| Final Cut Pro | 12.3 | Frame.io workflow extension; To Do markers | remarks come back NOT as data but as a video file: "Import comments from Frame.io by dragging the Comment Clip to Final Cut Pro. This creates a flattened file that can be moved around on the timeline." The return path is honest: a to-do marker is exported as a comment |
| After Effects | 26.x | Frame.io panel; the Comments column | a remark arrives in the composition as a text layer and a service line in the Comments column — that is, AE has no place of its own for someone else's remark, and it is put into the first available one |

**Outside the core.** Without OTRN this feature cannot be described: it is the only place where the address of a remark is DESCRIBED as a schema, not as the behavior of a particular panel, and where it is stated plainly what the address lacks in order to survive the next version.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OTRN (Open Timecode-Related Notes) | 1 (November 2025) | `notes[].time` / `timecode` / `frame` / `noteId` | a note has THREE time addresses at once: a mandatory `time` in seconds from the start, an optional `timecode` in `HH:MM:SS:FF` and an optional `frame`; in addition to them — `noteId`, "a way to give a note a unique ID. This can be used to track notes across multiple versions." The format was introduced to "resolve common issues with other formats, such as CSV, XML and OTIO" |

## The trick behind it

**1. A remark addresses not the edit but an IMMUTABLE file — and lives exactly as long as that file lives.** Frame.io attaches a comment to the uploaded version of the video, not to the sequence. Two consequences follow, which are in none of the lists of features. First: the remark physically cannot lie — the file to which it is attached cannot be re-edited. Second, paid for by the first: it does not move to the next version, and the help suggests carrying the remarks over by COPY-PASTE — "copy the comments to your clipboard to paste into another clip." The immortality of the address is bought with the mortality of the remark itself at every new render.

**2. The anchor works in BOTH directions, and this is seen as a defect.** Since the remark knows its version, the panel, while remaining open, pulls the edit toward that version as well: in Premiere, an open Frame.io V4 comments window rolled the markers of a duplicate of the sequence back to the state of the previous upload, and the only workaround was to close the window altogether. In V3, imported markers became ordinary sequence markers and "would adjust to the edit as normal" — that is, the transition to "live connection" took from the editor the right to move someone else's mark with their own edit. The difference is not in the quality of implementation but in who owns the address.

**3. A remark actually has two addresses, and only one crosses the editor boundary.** Besides time, a comment carries a place IN THE FRAME: the pin of an anchored comment, the lasso of an annotation. Precisely these two kinds do not return to the Premiere panel at all — that is, half of the reviewer's remarks (the one where they pointed a finger at a corner of the frame) exists only in the browser. And this is not a forgotten detail but a consequence of the design: a spatial address is not expressed by a mark on a time ruler, and the NLE has no other place for it.

**4. The industry answer to all this has already been formulated — and it is about identity, not about time.** OTRN v1 adds to the note a `noteId` precisely "to track notes across multiple versions." That is, the market's conclusion after ten years of cloud review: an address by time is needed to SHOW a remark, and it is not suitable for RECOGNIZING it.

## Pros

- The address cannot go stale: the video's version is immutable by construction, so "at 4:12" points to the same frame even a year later.
- The reviewer needs nothing but a player: they do not open the project, have no access to the material, and cannot break anything.
- The remark and the edit are decoupled in time: the edit can be carried on while the review is in progress, without fear that someone else's mark will move away together with the clip.
- Two-way linking (Resolve Presentations) removes manual import: the remark appears in the project as a marker at once, and the "done" mark is visible to the reviewer.

## Cons

- A remark does not move to the next version — and a video has dozens of versions. The official answer ("copy the comments") is manual work that grows linearly with the number of remarks.
- The live connection takes away the right to move a marker: while the panel is open, the address belongs to the upload, not to the edit, and the edit silently rolls back to it.
- The spatial half of a remark (annotation, pin) does not return to the editor at anyone's — the most visual part of the review stays in the browser.
- The address is computed in the coordinate system of the VIDEO: a review of a 23.976 video and a 25 fps timeline give different frames for identical numbers.
- All of this works only with an uploaded video: the price of the address is a whole render and the upload of gigabytes before the first word is said.

## What users say

About the fact that the anchor pulls in both directions — a direct complaint about V4:

> After I've imported marker comments about a sequence from Frame.IO, if I open a
> duplicate of that sequence again while having the Frame.IO comments window
> open, the comments in my timeline will revert to match the previous version of
> the edit.
> — Andrew C. Brinsmead, 28.10.2025,
> https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768

The product manager of the panel replied in the thread on 30.10.2025 that the V4 panel is being replaced by a complete rebuild of the integration in Premiere 25.6 — that is, the mechanism was acknowledged as unsuccessful by the vendor itself, not only by users.

## How it fits Kinotrain

### What already exists

Half of the mechanism works with us and has worked for a long time — the half that is about the ADDRESS.

- The project marker: `Project.markers` (`src/libs/kt-model/src/project.rs:458`) — a flat list sorted by time. The address is a tick of the TIMELINE.
- The marker itself — `Marker { t, duration, name, color }` (`src/libs/kt-model/src/project.rs`, "marker identity — by `t`"); the range form came from OTIO `marked_range` (R.24). The user explicitly asked for the range to be treated as primary: "in the editor I always did it myself with two markers, but used them essentially as marked_range" (`docs/user-feedback.md`).
- The address works as navigation: markers are magnetic targets and scrub points (`src/apps/kt-app/src/app.rs`, the assembly of `snap_targets`); a double click on the strip of a range marker sets a region by its boundaries (t170, `layout_tests.rs`).
- The name of a marker is written on the time-label row, not in its own band — the decision of item 3 of R.12, the reference for UI rule 13.
- The foreign markers of the root Stack are read and written back (`src/libs/kt-otio/src/convert.rs`); the OTIO color is stored specifically for the sake of the round trip — `color: Option<String>`.

And exactly here the mechanism ends.

- **A remark has no text.** A marker has one string field — `name`. A `comment` field does not exist in the model, although it exists in the OTIO schema `Marker.2` (`src/libs/kt-otio/src/schema.rs`): our reader discards it, the writer writes it empty. A foreign `.otio` with remarks loses them at the first save.
- **There is no version of the document.** Only versions of the FORMAT travel into the file (`FORMAT_VERSION` in `src/libs/kt-model/src/project.rs`); `model_rev` is the key of drawing caches, `saved_rev` is the guard of autosave, and neither reaches the disk. So "this remark is about the third variant of the edit" cannot be expressed by anything with us.
- **Nothing moves a marker.** In `enum Command` there is not a single operation that would shift `markers` when clips are edited (`src/libs/kt-model/src/model.rs`), and we have no ripple edits at all. Premiere's "Ripple Sequence Markers" is enabled by default: a marker follows the clip on insertion, deletion, gap closing and trimming. With us a marker stays where it was placed, always.

OFFLINE: DEGENERATES — `src/libs/kt-model/src/project.rs:458` — the address remains, the statement disappears: the project's marker list stores time, duration, name and color; a marker snaps magnetically, one jumps to it and sets a region by it, so "here" on one machine is expressed precisely and repeatably; but a marker has one text field, which also serves as the address of named ranges, the version of the document is absent from the file altogether, and the remark degenerates into a nameless flag whose meaning is held in the head of the person who placed it.

NETWORK COST: both sides are obliged to look at ONE and the same immutable video — media address — the remark does not carry a frame, it refers to one, and "at 4:12" is resolved only where exactly the same version lies; without a shared copy it is a string with a colon, and there is nothing to compare one's own version against another's — the revisions of the document are absent with us both in the file and in the model.

### What is missing

- Text for a marker: a second string field which does NOT take part in the namespace and therefore can hold any phrase (analysis — F-COL-022).
- A "done" mark — a state of the remark; today a marker either exists or does not.
- A way to say to which state of the edit a remark refers: not a network, but a revision of the document that can be written into the file.

### What blocks it

The main obstacle is not technology but an accepted decision. In R.35 (item 4 of `docs/user-feedback.md`) the option "introduce real ids" for markers was REJECTED: "markers today are addressed by time, and translating all five marker commands to ids costs more than the uniqueness of names." Review is the only scenario that reopens this question, because for a foreign remark an address by time does not work by construction (OTRN answers this with `noteId`). This note records the connection and does NOT propose changing the decision: a new request from the user is a mandatory condition.

The second obstacle is space. The text of a remark is an order of magnitude longer than a marker name, and UI rule 13 forbids creating a band for a caption that appears occasionally; UI rule 7 requires measuring the gallery and truncating (`widgets::elided_name`), not hoping for something short.

### Which invariants it touches

None of the twelve: a marker takes part neither in the composition, nor in the audio track, nor in the cache. It touches UI checklist rule 13 (the text of a remark must seek an ALREADY occupied place, not a new band) and rule 7 (long text is truncated, not allowed to break the time-label band).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: text on a marker in the model and the format; display of text in the UI and in the list |
| Where the main work is | kt-model (the second text field of a marker), kt-otio (`comment` of the schema `Marker.2` stops being lost), kt-app (display and editing of the text) |
| Project format | a version is needed: the field is additional, but an old reader would lose it silently — the R.29 rule |
| Tests that will have to be written | the `.otio` round trip: a foreign marker with `comment` comes back with the same text (an extension of t144/t148); a layout test "a long remark text does not grow the time-label row"; a parity test "a remark survives copy-paste of a batch" next to t249 |
| Depends on | F-COL-022 (the author and time of the remark) — there it is also analyzed why a marker name does not suit as text |
| Breaks compatibility | no |

## Verdict

We take the local half and do not introduce the server half: there is no second participant, while a field for the text of a remark is useful even for one person — today the editor writes "re-shoot" into the name of a marker, which is at the same time the address of a named range.

What we take right now: `comment` of the schema `Marker.2` stops being lost on the round trip (a pure defect, not a feature), and a marker gets text separate from its name. What we do NOT take: an identity of a marker beyond time — the decision of R.35 item 4 stands by the user's direct request and is not revisited without a new request; and a version of the document — it belongs to F-PRJ-002.

Boundaries: what is paid for two remarks on one frame — F-COL-021; who said it and when — F-COL-022; why half of a review lives on a clip — F-COL-023; color and name of a marker as event data — F-UI-005 (there the subject is the fill and its owner, not the statement). We differ from F-TL-007 by not introducing a second entity: its verdict "we do NOT introduce a guide as a separate entity, our marker is already a timeline anchor line" continues here literally — the remark is placed ON the marker, not next to it.

## Sources

- https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768
  — Frame.IO (V4) keeps overwriting comments markers in Premiere Pro; the reply of the panel's product manager of 30.10.2025 (access date 2026-09-08)
- https://help.frame.io/en/articles/9859849-adobe-premiere-frame-io-v4-comments-panel-overview
  — what the panel does NOT do: annotations, anchored comments, attachments, downloads (access date 2026-09-08)
- https://help.frame.io/en/articles/1711085-how-to-copy-and-paste-comments-legacy
  — the official answer to "comments do not move to the new version": copy-paste (access date 2026-09-08)
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration
  — Presentations: "Shared markers and comments are automatically and bidirectionally relinked with DaVinci Resolve" (access date 2026-09-08)
- https://support.frame.io/en/articles/2444494-take-the-fcp-tour-legacy
  — comments in Final Cut arrive as a Comment Clip, a "flattened file" (access date 2026-09-08)
- https://larryjordan.com/articles/premiere-pro-timeline-marker-secrets/
  — Ripple Sequence Markers is enabled by default, 06.09.2020 (access date 2026-09-08)
- https://otrn.editingtools.io/ — OTRN v1 (November 2025): the fields `time`, `timecode`, `frame`, `noteId`, the declaration "resolve common issues with other formats, such as CSV, XML and OTIO" (access date 2026-09-08). No key was created in `_editors.md`: this is a format, not an editor, and the registry is updated by a wave, not by a note
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/using_markersregionscommands.htm
  — markers, regions and command markers of VEGAS (access date 2026-09-08); the VEGAS row, in the part "there is no review service of its own," is marked `[NOT VERIFIED]` for 2026.0 — the help is published for line 22
- Our code checked by grep over `src/` on 08.09.2026: `kt-model/src/project.rs`
  (`Project.markers`, `Marker`, `FORMAT_VERSION`), `kt-model/src/model.rs`
  (five marker commands, none of them shifting), `kt-otio/src/schema.rs` (`Marker` with `comment`), `kt-otio/src/convert.rs` (reading and writing of Stack markers), `kt-app/src/app.rs` (magnet, region by marker); `docs/user-feedback.md`, R.35 item 4
