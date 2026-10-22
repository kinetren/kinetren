---
id: F-COL-012
title: Track owner
status: candidate
priority: B
cost: M
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: [7, 13]
round: null
point: null
tests: []
depends_on: [F-TL-006]
trackers: []
services: [team-projects, bm-cloud, resolve-project-server, fcp-shared-library]
checked: 2026-09-08
---

# F-COL-012 — Track owner

## Essence

Work shared by two people is divided not by projects but by tracks: "V1 and V2 are my edit, A3 is the voice-over, don't touch it, A4 is music, the other person puts it there." This is the most frequent and cheapest form of dividing labour in editing, and it rests on nothing: the program can lock a track (a state anyone can remove with one click) and cannot say WHOSE it is. In real work the carrier of the agreement turns out to be the track name.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | there is NO lock on a track; there is Switches → Lock for EVENTS | a track can be locked only by selecting all its events to the very end and locking them in one batch; users have been asking for a Lock button in the track header for many years, and there is room for it in the button set |
| Adobe Premiere | 26.x | Toggle Track Lock (a lock in the header of each track) | a flag on a sequence track; in Team Projects the same flag on audio tracks works unreliably — that is, the mechanism that is supposed to carry the agreement does not even hold its own state |
| DaVinci Resolve | 21 | Lock Track; in shared work — locking of the BIN and of the TIMELINE | a track lock and claiming an object in a shared project are two different things: one can claim a bin or a timeline as a whole, but not a track |
| Final Cut Pro | 12.3 | no tracks at all; instead Roles | a role is a semantic name assigned to a CLIP, lives in the library, travels in XML and controls both display and export stems; closest of all to "name as data", but it does not know its owner either |
| After Effects | 26.x | lock on a LAYER (icon in the Timeline panel) | the same flag, one level lower: on a layer, not on a track; a composition has no tracks, so work has to be divided by compositions |

## The trick behind it

**Everyone can lock a track and nobody can assign it an owner, and the difference between these two things is not in the amount of code but in the fact that a lock is a state WITHOUT AN AUTHOR.** Anyone removes it with one click, and after removal no trace remains in the project: neither who locked it, nor why, nor whether it was locked at all. That is why in live projects the role of owner is played by the track NAME — "VO — DO NOT TOUCH", "MUSIC (Anya)", "LOCKED — final mix". That is, the only carrier of an agreement between two people is a string that the program treats as a label and does not read.

**Second layer: the granularity of occupation is the same for everyone and equals one RECORD, not one edit, and a track is absent from every list.** Team Projects occupies a sequence, Resolve — a bin and a timeline, Final Cut — the library as a whole. Between "the whole edit" and "a clip" the track is the most natural unit of conversation, and exactly that is supported by nothing: it is not a record in any of these databases. Practice divides by tracks, while programs can divide only by files — hence the whole mechanism has moved into names.

**Third layer: Final Cut, the only one that abandoned tracks, by accident solved half the problem.** A role is a name, but one that lives in the document: it is assigned to a clip, survives re-editing, and determines the export stems. Once the name became data, it stopped being a label. A role still does not know its owner, but it shows what has to change so that the agreement stops depending on who presses the lock first.

## Pros

- A name as the carrier of an agreement requires neither a user account, nor a server, nor access rights: it travels in the file and is readable by a person. This is the most portable mechanism of all those used in real work.
- The lock is cheap and understandable: one press, visible at a glance, cancelled instantly. As protection against one's OWN error ("don't move, I'm finished here") it works flawlessly, and it is in that capacity that it is used most often.
- Final Cut's roles show that turning a name into data pays off through a second use: the same name starts controlling stem export, so the agreement and the result stop diverging.

## Cons

- The lock does not preserve intent: it is removed by the same click that set it, and remembers nothing. The program does not distinguish between "locked because it is edited" and "locked because it belongs to someone else."
- A track name as a carrier works exactly until the first reordering of rows and the first import: the label takes part in no check, so two tracks named "VO" are a legitimate state for everyone.
- The mechanism the agreement relies on is unreliable even where it exists: in Team Projects the audio track lock does not hold, and people have been reporting this for the fourth year.
- In models without tracks (Final Cut, After Effects) one has to agree in a different currency — roles or compositions — and the agreement does not carry over between pipelines: "my A3" means nothing in a project without tracks.

## What users say

The mechanism that is supposed to carry the agreement does not hold its own state — and breaks precisely in shared mode:

> Whenever I try to lock a track the button gets stuck like this and doesn't
> actually lock.
> — zack_3303, 30.10.2024,
> https://community.adobe.com/t5/premiere-pro-bugs/p-audio-tracks-won-t-lock-or-unlock-individually-can-only-lock-or-unlock-all-of-them-at-once/idi-p/14953063

> The issue seems only to affect Team Projects.
> — floby2k1, 28.03.2025, the same thread (confirmed by Bryan Dumaguina,
> 24.09.2025: "It only happens in Team Projects")

In VEGAS the request is the reverse — people ask for the mechanism itself, which does not exist: the thread "locking a track" on the VEGAS Creative Software forum
(https://www.vegascreativesoftware.info/us/forum/locking-a-track--64946/) and "Locking tracks in Vegas?" on Creative COW
(https://creativecow.net/forums/thread/locking-tracks-in-vegasae/) boil down to one workaround: select all events of the track to the very end and lock them in one batch.

## How it fits Kinotrain

### What already exists

- A track name exists and travels in the file: `UiState.lane_names`
  (`src/libs/kt-model/src/project.rs`, type `Vec<(LaneKind, u32, String)>`), with a comment stating directly "Outside undo, like all of UiState." Empty — the label follows the V1/A1/M1 formula.
- Moreover, we put the name of video and audio tracks into the STANDARD OTIO field `Track.name` (`src/libs/kt-otio/src/convert.rs`), not into our own section: a foreign editor sees it and preserves it. Only the names of morpher and camera tracks, for which OTIO has no place, remain in `kt.ui`.
- The document already contains a named entity that addresses tracks: `MarkedArea` (`src/libs/kt-model/src/project.rs`) — a name, a time, a set of tracks. The model says of it verbatim "The name is an ADDRESS: non-empty and unique"; its track keys are SEMANTIC (reordering rows does not move the area); it goes through undo and travels in the file (`areas` in the kt envelope, schema v4).
- There is a precedent for promoting a field from convenience into the document, with a known price: `lane_order` moved from `UiState` into the model in R.35, when row order began to affect the picture (the morpher acts "on everything below"); in the file it now lies next to `ui`, the old field remained only as input (`#[serde(default, skip_serializing)]`), and the file was raised to v4.
- The only word `locked` in the whole project belongs to `BinGroup` (a group of cards in the media bin). A track lock does not exist at all: mute/solo/lock is F-TL-006, and there `we_have: no`.

OFFLINE: DOES NOT REMAIN — `src/libs/kt-otio/src/meta.rs` — nothing of the owner remains, and the place where this is visible is the file envelope: the `ui` field is declared a CONVENIENCE and stands exactly between `lane_order`, which in R.35 was moved out of it into the model as soon as it began to affect the picture, and `areas`, whose name is an address. The only carrier of the agreement, the track name, lies in the convenience, and today a person writes "don't touch" directly into the label.

NETWORK COST: the second person must have a name that the program knows, not a letter — access rights — an owner without a list of people degenerates into a string, and that string is already carried today by the track name; so without accounting for people and their access rights, the new entity adds nothing except a place to write the same thing.

### What is missing

- Not an "owner" field — we do not introduce it (there is no second participant) — but making the NAME a document: today it is outside undo (rename and change your mind — nothing to roll back to), outside validation (two "VO" are legitimate), and outside any checks.
- A track lock as such (F-TL-006): we cannot forbid an edit even to OURSELVES, so "don't touch" is backed by nothing but attentiveness.
- A way to see the agreement where it is violated: the track name is visible in the row header, while the gesture happens on the clip.

### What gets in the way

`UiState` as a whole is declared a convenience and deliberately does not go through undo — this is a decision, not an oversight, and it protects undo from junk such as the scroll position. So moving the name out is not "moving a field" but splitting the structure in two: the convenience stays, the name moves into the model, the file gets a version (as v4 did for `lane_order`), and reading old files must keep lifting the name from `Track.name`. Plus layout rules: a long track name is already drawn today as a label over the beginning of the track (UI rule 13 — do not waste space; UI rule 7 — the text fits, t102/t255), and any "name + owner" must fit into the same strip rather than create a new one.

### Which invariants it touches

None of the twelve: the track name participates in neither the composition, nor determinism, nor the cache. It touches two rules of the UI checklist — 13 (do not create a new strip for something that can be placed on an occupied spot) and 7 (a long name is truncated, not allowed to break the layout) — and it requires a new row in the `reach_tests` registry table, t253, if the track gets one more verb.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: the name from `UiState` into the model (with a format version and undo), then the track lock as a separate state |
| Where the main work is | kt-model (the name as a document field, uniqueness validation, rename commands through undo), kt-otio (envelope version, reading old files from `Track.name`), kt-app (the track header, the label, the verb registry) |
| Project format | a v6 version is needed by content: an old build reading the name from `Track.name` would not lose it — so a hard refusal is NOT needed, a conditional bump is enough, like the cameras in R.29 |
| Tests to write | a round trip of the name through the model and undo; "two identical names are an error of the command, not a silent acceptance"; reading a v4/v5 file where the name lies in `Track.name`; a layout test "a name with an owner's mark does not grow the track strip" |
| Depends on | F-TL-006 (the track lock — the neighbouring half of the same conversation) |
| Breaks compatibility | no: the name, as it lay in `Track.name`, still lies there |

## Verdict

Take the local core — **move the track name from convenience into the document** — and do not take the owner: declaring a second participant is not a note's right, and an owner without a list of people and rights is the same string as today, only in a different field. The move pays off even alone: a name that goes through undo and is checked for uniqueness is fit for an address — exactly like `MarkedArea`, where this is already written into the model.

How we differ from neighbours. **F-TL-006** (track switches — mute, solo, lock, "hide") concerns what the switch does TO THE PICTURE AND THE SOUND for one person: the lock protects against one's own slip, mute and solo change what is heard. Here the subject is different: not a state but a BELONGING, and it must survive the removal of a lock — otherwise the agreement disappears together with one click. The pair is asymmetric: F-TL-006 — cost L and pure UI, this one — cost M and a pure model. **F-PRJ-001** (several edits and a shared bin) concerns how to divide work by the STRUCTURE of the document; a track is valuable precisely because nothing needs to be divided. **F-UI-005** (clip colour as data and as decoration) asks the same question, "label or data," but about an event and about distinguishability, not about agreement.

## Sources

- https://community.adobe.com/t5/premiere-pro-bugs/p-audio-tracks-won-t-lock-or-unlock-individually-can-only-lock-or-unlock-all-of-them-at-once/idi-p/14953063
  — the track lock does not hold, and only in Team Projects (accessed 2026-09-08)
- https://www.vegascreativesoftware.info/us/forum/locking-a-track--64946/ —
  the request for a Lock button in the VEGAS track header (accessed 2026-09-08)
- https://creativecow.net/forums/thread/locking-tracks-in-vegasae/ — the workaround through "select events to the end → Switches → Lock" (accessed 2026-09-08)
- The occupation granularity of services — `docs/features/_services.md`, column "Occupation unit": a sequence (Team Projects), a bin and a timeline (Blackmagic Cloud and Resolve Project Server), the library as a whole (the shared Final Cut library). A track is absent from every row.
- The Final Cut roles are described from Apple's documentation; the table row reflects the composition of the mechanism (a role is assigned to a clip and controls the stems), not verified live — no public tracker exists for any of the five (`docs/features/_trackers.md`, "Whom we do NOT take").
- Our code was checked by grep over `src/` on 08.09.2026: `kt-model/src/project.rs` (`UiState.lane_names`, `MarkedArea`, `BinGroup.locked`, `lane_names_*`), `kt-otio/src/meta.rs` (`KtTimelineMeta.ui`, `lane_order`, `areas`), `kt-otio/src/convert.rs` (the track name in `Track.name` and back), `kt-model/tests/parity_ops.rs` (t90).
