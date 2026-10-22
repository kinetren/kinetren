---
id: F-COL-028
title: Chat inside the editor
status: rejected
priority: C
cost: S
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app]
invariants: [12]
ui_rules: [10, 12, 13]
round: null
point: null
tests: []
depends_on: [F-COL-027]
trackers: []
services: [resolve-project-server, bm-cloud, frame-io]
checked: 2026-09-08
---

# F-COL-028 — Chat inside the editor

## Essence

In Resolve's collaborative mode there is a chat window: people write to each other without leaving the program — "look at the second take," "I took the timeline," "I put the music in." This is the only collaborative function that does not touch the document AT ALL: neither a clip, nor a track, nor a marker, nor a frame. It is also the only collaborative function for which not a single breakage report has been found — although the neighboring mechanisms (locks, merging, return of comments) break for years and publicly.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no collaboration service of its own at all (`_editors.md`, the section on the cloud binding of the five): a second participant does not exist in the program, so there is no interlocutor either |
| Adobe Premiere | 26.x | no chat; there is presence and Frame.io comments | Team Projects shows which of the invited are online now, but provides no correspondence — they agree in an external messenger. The conversation about a frame arrives from Frame.io as comments and is WRITTEN INTO THE DOCUMENT as markers; that is also where it breaks |
| DaVinci Resolve | 21 | Collaboration Chat | the button appears only after the collaborative mode is switched on and is highlighted when a new message arrives; the correspondence is tied to the project, that is, it lives in the project library (its own PostgreSQL or Blackmagic Cloud), not in the timeline. Requires Studio; the free build has no collaborative mode |
| Final Cut Pro | 12.3 | none | there is no cloud at all, only a shared volume and an exclusive lock on the whole library at once: one person at a time is in the document, and there is no one to talk to inside the program |
| After Effects | 26.x | no chat; the same Frame.io panel | the same as Premiere: the conversation channel is moved out into the service, and its traces return into the document as markers |

**Outside the core.** Four of the five core rows are "none," and the mechanism cannot be explained by the five: the chat exists for exactly one vendor, and the MEANING of its design is visible only next to its own opposite — the same conversation, moved INTO the document. Both halves are supplied by services, not by editors, which is why the rows are here and not in the table above.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blackmagic Cloud Presentations | as of 08.09.2026 | chat + shared markers | one vendor supplies both halves at once and makes them DIFFERENT: the chat is declared as the place for "persistent notes and comments" and lives outside the timeline, while markers and comments are synchronized with the Resolve timeline in BOTH directions within seconds. The conversation outside and the conversation inside the document are not the same thing here, and the difference is deliberate |
| Frame.io (panel in Premiere/AE) | V4 | Comments → sequence markers | the same second half at Adobe, taken to the end: the service's comments become markers IN THE DOCUMENT. It is exactly this half that overwrites the user's own markers (quote below), whereas not a single complaint was found about the Resolve chat |

## The trick behind it

**1. The chat is the only collaborative mechanism that lies OUTSIDE the unit of occupancy.** The unit of occupancy in Resolve is the bin and the timeline (`_services.md`, the column "Unit of occupancy"); a remark is neither of these, so it need not be locked, merged, or conflict-resolved over. Hence its reliability, and it is not an achievement of the implementation: at neighbors with the same function, the "Resolve Conflicts" dialog in Premiere has been empty for a fourth year, while the chat simply has no subject of dispute. A collaborative function breaks exactly to the degree that it writes into the document.

**2. The same vendor supplies both halves, and they behave differently.** In Blackmagic Cloud Presentations the chat is "persistent notes and comments" outside the timeline, while markers are synchronized with the timeline in both directions. The half that enters the document is the only one that can spoil the document; at Adobe exactly this half (the return of Frame.io V4 comments as Premiere markers) wipes the user's own markers and rolls them back to the previous version of the video. The "outside / inside" boundary here empirically coincides with the boundary "never broke / breaks publicly."

**3. The cheapest function is locked behind the most expensive infrastructure.** The chat button appears only after the collaborative mode is switched on, and that requires a shared project library — its own PostgreSQL server or a paid cloud — plus shared storage and a Studio license. That is, the mechanism that needs nothing from the document is available only to those who bought everything. This is what explains why the form "a conversation next to the document" has not been replicated in the industry: nobody sells it separately, although it is the cheapest thing in the whole list.

## Pros

- Zero cost for the model: no lock, no merge, no format version, no object identifier are needed — the only collaborative function that has no requirements on the document.
- It does not spoil the work with a question: to ask "is it exactly so here?" one does not have to place a marker in someone else's timeline.
- It promises nothing about bytes: the chat participates in neither the composition nor determinism, so it cannot diverge from the render.
- Context stays with the work — the correspondence is tied to the project, not to an external messenger (a claim of the vendor and of secondary reviews; there is no way to check it live).

## Cons

- The value lies entirely in the second person. Without them it is a notepad, and the worst kind: without search, without files, and without formatting.
- A conversation without an address: a remark is tied neither to a frame nor to a piece, so "redo it here" has to be described in words. Precisely because of this, a second mechanism is set up next to it — comment-markers — and it is that one which breaks.
- Even where a chat exists, it is measured by what is missing: the 2026 complaint is not "the chat is bad" but "there is no video link," that is, text correspondence is recognized as an insufficient form of feedback.
- The surface must live at REST: a message arrives without a single action by the user. For an interface where redrawing is tied to motion, this is the first source of change without input in the program.
- It is locked behind infrastructure (trick 3) and behind the paid build, that is, half of the users of the product do not have it at all.

## What users say

What happens when a conversation enters the document:

> Frame.IO V4 is overwriting & deleting comments markers on my sequences in
> Premiere Pro, and I need it to stop making my comments markers revert to the
> previously saved version on Frame.IO
> — Andrew C. Brinsmead, 28.10.2025,
> https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768

How the chat that does exist is measured:

> There's also no integrations that allow you to chat face-to-face while you
> work, which makes giving and receiving feedback a much slower process.
> — Evercast, "How to collaborate in DaVinci Resolve," 26.02.2026,
> https://www.evercast.us/blog/how-to-collaborate-in-davinci-resolve

Separately and honestly: **no complaints about the Resolve chat itself could be found**, and this is part of the finding, not a gap in the reconnaissance. The search was made in three places: `forum.blackmagicdesign.com` (403, only search snippets available), `liftgammagain.com` (the only suitable thread — about configuring the collaborative environment — does not touch the chat), `community.adobe.com` (Adobe has no chat at all, so there can be no complaints). A mechanism about which there is nothing to say is one in this category.

## How it fits Kinotrain
CAPABILITY: openspec/specs/project-format/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Rejected: there is no discussion next to the document — there is no network in any form, and comments without a second party degenerate into markers.
OFFLINE: DOES NOT REMAIN — `src/apps/kt-app/src/meters.rs:55` — from the chat, for one person, neither a remark nor an interlocutor remains, and the place where this would be visible is our only surface next to the document: the meters panel never touches the model across the whole file, and yet it DELIBERATELY goes dark when the cursor is placed. There is not a single surface in the program that is obliged to live at rest, and a "notepad in the corner" does not become one: it has neither a second author nor a reason to change on its own.
NETWORK COST: a remark must have an author and an order — the user account — neither the document nor the application contains a person's name or identifier, so a chat line for one participant is indistinguishable from a journal line, and there is no one to put two remarks in order.

## Estimate

| What | Answer |
|---|---|
| Size | S — the panel is the entire visible part; all the remaining cost lies outside this note and is filed not by it |
| Where the main work is | kt-app (the panel and the notification); the model, the format and the engine are not touched at all — and that is the main property of the feature |
| Project format | not touched: the correspondence is not a document |
| Tests that will have to be written | none — the feature is not taken; from the rejection follows a requirement for the neighbors, not a test |
| Depends on | F-COL-027 (there it is also analyzed what of a conversation nevertheless becomes data) |
| Breaks compatibility | no |

## Verdict

**Rejected.**

**The reason is named by numbers.** Invariant 12 ("repaint only on MOTION," tests 20 and 38) plus rules 10, 12 and 13 of the UI checklist. The chat is the first surface obliged to live at rest: a message arrives without a single action by the user, a notification about it asks for a banner that we do not have, and a permanent strip under rare text is forbidden by the direct text of rule 13. Our only "next to the document" is the meters panel — and it is done exactly the opposite way: `meter_holds_level` (`src/apps/kt-app/src/meters.rs:55`) switches off even the instrument that has something to show, and this was the user's decision (item 4 of R.16), not a saving.

**What is NOT the reason for the rejection** — so that a year from now nobody argues about the wrong thing. Not the cost: `cost: S`, a panel with an input field is made in one stage, and a rejection resting on size would be a fraud. Not "the editor does not need to talk": Resolve proved the opposite — this is its only collaborative function on which not a single complaint was found. And **not the absence of a network**: this argument in the `collab/` category is forbidden by rule 13 of the README deliberately; it is known before the reconnaissance, and here it is the weakest — a network would add an interlocutor to the chat, but would not remove a single one of the four points above.

**The cheap equivalent of the same value is not a function but a lesson of form, and it has an address.** The value of the chat is not in the correspondence, but in the fact that the conversation lies OUTSIDE the unit of occupancy: it need not be locked or merged. For us this is the acceptance rule of the neighboring notes of the category: **everything that comes from a second person enters us as DATA, which never writes into the timeline by itself.** The cost of violating this has been measured by other people's hands — Frame.io V4 wipes the user's own markers (quote of 28.10.2025), and Blackmagic separated the same two halves deliberately: the chat outside, the markers inside. The nearest application of the rule is F-COL-027: the acceptance state is taken as a type in the model and is set BY THE HAND OF THE OWNER of the document, rather than arriving from outside and rewriting what a person has set themselves.

**The condition for reopening is exactly one, and it is not "a network appears."** The note is reopened if we acquire at least one surface obliged to live at rest — then a chat becomes a small addition to the already accepted price. Today there are zero such surfaces, and the one closest to them in meaning is deliberately switched off.

## Sources

- Blackmagic Design, Collaboration page — verbatim about the chat: "DaVinci Resolve features built in chat so team members can talk about shots, review tasks and share creative ideas without leaving the software," and about locks: "Automatic bin and timeline locking let multiple people work without overwriting each others work":
  https://www.blackmagicdesign.com/products/davinciresolve/collaboration
  (access date 2026-09-08)
- Adobe Product Community, "Frame.IO (V4) keeps overwriting comments markers in Premiere Pro," Andrew C. Brinsmead, 28.10.2025:
  https://community.adobe.com/t5/premiere-pro-bugs/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro/idi-p/15565768
  (2026-09-08)
- Evercast, "How to collaborate in DaVinci Resolve," 26.02.2026 — about the absence of video link and about the presence of a text chat:
  https://www.evercast.us/blog/how-to-collaborate-in-davinci-resolve (2026-09-08)
- The device of the chat button itself (it appears after the collaborative mode is switched on, is highlighted on a new message, the correspondence is tied to the project) is taken from secondary reviews — Fastio, "DaVinci Resolve Collaboration — Team Setup Guide 2026" (https://fast.io/resources/davinci-resolve-collaboration/) and the cited Evercast article. The primary source is physically unavailable: the mirror of the Resolve manual on `steakunderwater.com` returns 404, `forum.blackmagicdesign.com` — 403. The mark `[NOT VERIFIED]` refers precisely to these details, not to the existence of the chat, which is confirmed by the vendor's page
- Blackmagic Cloud Presentations (the chat as "persistent notes and comments" plus two-way synchronization of markers with the Resolve timeline) — according to the description of Blackmagic Cloud and secondary reviews, not checked live:
  https://help.cloud.blackmagicdesign.com/intro/what-is-blackmagic-cloud/
  (2026-09-08)
- The absence of a chat in Team Projects and the presence of a presence indicator — according to secondary guides (Evercast, LucidLink); `helpx.adobe.com` is unavailable, see `docs/features/_services.md`, section "Availability of sources on 08.09.2026"
- The unit of occupancy of services (bin and timeline for Resolve, sequence for Team Projects, the whole library for Final Cut) — `docs/features/_services.md`
- Our code checked by reading and grep on 08.09.2026: `src/apps/kt-app/src/meters.rs`
  (`meter_holds_level`, `SCRUB_SHOW_MS`, absence of accesses to the model),
  `src/apps/kt-app/src/theme.rs` (`tooltip_delay`), `src/apps/kt-app/CLAUDE.md`
  (rules 10, 12, 13 of the checklist)
