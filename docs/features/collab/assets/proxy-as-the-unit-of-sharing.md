---
id: F-COL-005
title: Proxies made not for oneself
status: researched
priority: C
cost: L
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: no
crates: [kt-app, kt-cache, kt-media]
invariants: [4, 8, 11]
ui_rules: [12, 20]
round: null
point: null
tests: []
depends_on: [F-MED-002]
trackers: []
services: [bm-cloud, premiere-productions, syncthing]
checked: 2026-09-08
---

# F-COL-005 — Proxies made not for oneself

## Essence

A proxy alone is a personal speed-up: you make it, edit, and throw it away. Shared between two people, a proxy stops being a speed-up and becomes a UNIT OF DISTRIBUTION: one participant makes the lightweight copies, and everyone edits on them, because nobody moves the originals over the network. From that moment the proxy is not a cache but second-class material: it has an address, it has to be handed to someone, and two people can make it at the same time.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Create Video Proxy | `.sfvp0` next to the source, with the same name. Distribution is not provided, but it happens by itself: the file lies IN THE MATERIAL'S FOLDER, so it travels with it — "distribution" here is a side effect of where it is stored |
| Adobe Premiere | 26.x | Create Proxies / Attach Proxies; Productions on top of a shared folder | proxies are ordinary files in the folder chosen at import. Others attach them by the name `<original>_Proxy.<ext>`. In a shared Production it is precisely the proxies that come apart: the path to them is recorded in the document and carries the volume name of THE machine where they were made |
| DaVinci Resolve | 21 | Proxy Media (portable files) against Optimized Media (internal cache); Blackmagic Proxy Generator; Blackmagic Cloud | the split is drawn exactly along the sign of distribution: Optimized Media "cannot be exported, and is not user accessible", Proxy Media is "fully portable and independent media". A separate free application builds proxies from a watch folder — that is, the role "the one who makes proxies for everyone" is pulled out into a separate program |
| Final Cut Pro | 12.3 | Proxy media, external storage location | proxies live either inside the library or in a separate storage location — and the second option exists precisely so that the derivative can be put where the second person can reach it. The lock is over the whole library at once |
| After Effects | 26.x | Set Proxy → File; Placeholder | a proxy is set on a specific project item, and the path is written into the document. Distribution is not provided: AE has neither a shared folder as a mode nor a proxy binding by a naming rule |

**Outside the core.** Without Syncthing one cannot say what exactly happens to two copies of one derivative: this is the only place where the outcome of a collision is formulated, not left to the implementation.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | sync-conflict | there is no merging: the file with the older modification time loses; on a tie, the device with the larger first 63 bits of its device ID wins. The loser is renamed to `<name>.sync-conflict-<date>-<time>-<who>.<ext>`, "are treated as normal files", and it spreads to all nodes as an ordinary file |

## The trick behind it

**1. It is not the material that conflicts, but the DERIVATIVE made from it — and this shows in where relinking is needed.** In a shared Production relinking is needed every time a file has been opened by the second participant, and only for proxies; the camera originals never come loose at all. The cause is not the bytes but the origin of the address: the path to the original came from the card and is identical on both machines down to the last segment, while the path to the proxy was written into the document by THE machine that made the proxy, together with the name of its own volume. The fix does not come from the editor: people rename the volumes so that the names match. That is, the only working way to distribute a derivative is to make its address stop depending on who made it.

**2. Files with the suffix `-conflict` appear IN THE PROXY FOLDER, not in the shooting folder, and this follows from the design.** The original is written once and is never rewritten — it has no two writers, so there is nothing to conflict over. A proxy, on the other hand, is GENERATED, and each person who needed it generates it: two people start building the same clip, both put the result under one name, and two-way sync must choose one of them by the more recent modification time, renaming the loser. The result is a pair of files that mean the same thing and do not match by a single byte (the encoder is non-deterministic in time and in threading), and both remain lying there: the sync system has no right to throw either one away.

**3. Blackmagic proves the same from the other end: over the network during a shoot, it is the DERIVATIVE that flies.** Live sync is built so that the camera records a second file — an HD proxy in H.264 — and it is that file that travels into the bin. The unit of distribution turns out to be not the material but its lightweight copy, and made not by the editor or even by an edit workstation. When the derivative becomes the thing that is exchanged, the question "who made it and will it match for two people" stops being an internal matter of the cache — which is how it looks in the list of capabilities.

## Pros

- An expensive transcode is done ONCE per group, not by each person: the saving is multiplied by the number of participants, which a personal cache cannot do at all.
- Proxies are ordinary files, so they are distributed by any means, including the one with no service: a shared folder, a disk in someone's hands, two-way sync.
- Separate roles arise by themselves: the "proxy builder" can be a separate machine and even a separate program (Blackmagic Proxy Generator over a watch folder), and the edit station is freed from transcoding entirely.
- Proxies from the site (camera ones) fit into the same scheme without a single change: for editing it does not matter who made the lightweight copy.

## Cons

- The derivative gets an ADDRESS — and from then on what breaks is not the material but the address: the volume name, the drive letter, the mount order. Relinking hundreds of clips becomes a regular operation, not an accident.
- Two people build the same thing and get different bytes: there is no deterministic encoder, so "the same derivative" for two people is a matter of trust, not of verification.
- Conflict copies do not go away on their own. Two hundred `-conflict` files are two hundred real files occupying space, and the program has no right to delete them.
- The truth about "what is already ready" is split: each participant has his own answer, and none of them is complete.
- Proxies are encoded lossy, so anything judged by fine gradations cannot be judged on the shared copy — yet it looks like material.

## What users say

The difference between material and derivative was named by a user directly, twice, in one thread:

> anytime either of us open a project after the other has worked in it, we have
> to relink or re-attach the proxy to the file paths on our respective drives.
> — J_Boy, Adobe Community, "proxy linking issues in shared production",
> 24.01.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/proxy-linking-issues-in-shared-production/td-p/15110876

> Premiere does not have the same issue with the camera original media, just the
> proxies.
> — J_Boy, same thread, 24.01.2025

The fix turned out not to be in the editor: the volumes were renamed identically, and afterwards the author confirmed that for source clips this was not needed at all — "this is not neccessary for source clips, only the proxies" (J_Boy, 29.01.2025, same thread).

What could NOT be quoted: complaints about 200+ `-conflict` files in the proxy folder of Blackmagic Cloud. The threads on `forum.blackmagicdesign.com` (in particular "Warning: Blackmagic Cloud Not Ready for Professional Use", `t=224635`, and "Help me get Davinci Resolve 19 Cloud (Proxies Only) Working", `t=214931`) do not open in this environment: Cloudflare returns 403 to both curl and the reader, and a paraphrase of search results is not a quote. The fact is given as secondary and carries no weight in the conclusion; the conclusion of the note rests on the Adobe thread, where there is an author, dates, and a confirmation of the fix.

## How it fits Kinotrain

### How this note differs from F-MED-002

F-MED-002 is about OUR OWN switch: make lightweight copies, edit on them, and on render silently return to the originals. Its main question is "how to guarantee that the export does not go out with proxies" (with us this is guaranteed by construction: the export opens the sources by `MediaRef::path` in `src/apps/kt-app/src/render_dialog.rs` and does not see the proxy file at all), and its verdict is "we take it when we hit the limit of 4K decoding".

Here the subject is different, and it begins where F-MED-002 ends: the proxy as a UNIT OF DISTRIBUTION. The questions of this note are whose address the derivative carries, what happens when two people have built it, and why precisely the derivative conflicts. From it follows exactly one requirement for F-MED-002, and it is cheap if set in advance: the address and identity of a proxy must be a function of the SOURCE, not of the machine that built the proxy. Retroactively this cannot be fixed — the thread about relinking in the shared Production shows exactly this: relinking is needed at every opening.

### What already exists

- **There is no proxy at all** (0 matches for `proxy` across all of `src/`), and this is not a gap on empty ground: there is `Quality { width, height }` — a request TO THE DECODER for the output size within ONE source, part of the cache key (`src/libs/kt-cache/src/lib.rs`, invariant No. 4). A proxy is another FILE behind the same media; that is a different axis, and F-MED-002 explains why the two must not be mixed in one key.
- **The only derivative of ours that lives as a file is the JPEG thumbnail strip.** It is made by the thumbnail worker (`src/apps/kt-app/src/thumbs.rs`) and written into `<project>.ktindex.db` next to the project. Peaks and the batch index live there as well.
- **The keys of this derivative are already machine-independent, and a test proves it.** `thumb_key_t` quantizes the moment FROM ZERO OF THE FILE on a fixed ladder `THUMB_STEP_LADDER`, not from `source_in` and not from the zoom (t62): two machines that open one file produce ONE AND THE SAME set of keys. So from the point of view of "will the derivative match for two people", the hardest part is already done with us — and it was done not for distribution but for the shared cache of the bin and the timeline.
- **Our other derivatives cannot be distributed by construction**: pre-FX bursts and post-FX frames live only in RAM and die with the process; `PostFxVideo{quality, fx_hash}` is moreover keyed by the hash of the effect chain, so it is useless to anyone else even if it arrived.

### What is missing

1. The derivative itself: a "file → file" transcoder does not exist (`export_mp4_with_backends` renders a COMPOSITION), and the model has no second path for media — see F-MED-002.
2. An address that does not depend on the machine. Today `.ktindex.db` keys media by its ABSOLUTE path (`ensure_media`, `src/libs/kt-media/src/index.rs`), so the derivative is tied to how the file is visible to me.
3. A store that lives between projects. The sidecar's DB belongs to the project: a second project for the same material will have its own, and a second person will have a third one of his own.
4. Robustness of the store against outside interference: `MediaIndexDb::open` on ANY opening error DELETES the file and creates it again. For a personal cache this is the right decision (cheaper to recompute); for a shared derivative it is the loss of someone else's work.

### What gets in the way

Invariant No. 4: the cache is the only source of truth about warmth. A shared store of derivatives inevitably creates a second truth ("it is not in my memory, but it is in the store"), and the coverage bars would have to choose which one to show — exactly the trouble for whose prevention the invariant was written. Second: our muxer snaps fractional frame rates to a whole grid (`grid_fps(23.976) = 24`), so a lightweight copy of hour-long material that we build would come out shorter than the original by almost four seconds — distributing such a derivative is out of the question, even more so than a personal cache. Third: invariant No. 11 requires byte determinism from OUR path, but it cannot require anything of a copy built by someone else's machine and someone else's encoder.

OFFLINE: DOES NOT REMAIN — `src/apps/kt-app/src/thumbs.rs` — there is nothing to distribute and no one to distribute to: we have no proxies at all, the decoded result lives in RAM and dies with the process, and the only derivative that exists as a file is the JPEG thumbnail strip, which the worker writes into the database next to MY project and addresses by absolute path. The absence shows precisely here: the keys of this strip are already the same on any machine, but the strip itself cannot be separated from a single document.

NETWORK COST: the unit of distribution — cross-project storage — there is no place to put the derivative so that two people can find it: our sidecar store is tied to one project, is keyed by an absolute path, and is deleted entirely on any opening error.

NETWORK COST: someone else's derivative as the source of truth — invariant 4 — a shared store would create a second truth about warmth next to the cache, and the coverage bars would have to pick which of the two to show.

### Which invariants it touches

- **No. 4** — the main one: a second truth about warmth is not allowed, so a shared store of derivatives must enter the cache through the same door as the decoder, not become a parallel answer to the question "is it ready".
- **No. 8** (scroll/zoom does not decode): the shared thumbnail strip is a direct amplification of this invariant, and that is precisely why it is our candidate for "a derivative for a second person".
- **No. 11** (determinism): someone else's lightweight copy is someone else's bytes. None of our comparisons against the reference make sense on it, and this must be said in advance, not discovered on the golden tests.
- UI rules: **20** (nothing grows by itself — derivatives are neither built nor distributed without a person), **12** (building is background work, not during a gesture or during playback).

## Estimate

| What | Answer |
|---|---|
| Size | L — and only together with F-MED-002: on its own, "distribution" is meaningless without proxies |
| Where the main work is | kt-media (the address of a derivative as a function of the source, not of the path), kt-app (who builds it and where it is put), kt-cache (one door to the truth about warmth) |
| Project format | does not need its own version — the place is standard, the `media_references` OTIO (analysed in F-MED-002); this note adds a requirement on the CONTENT of the key, not on the schema |
| Tests to write | the derivative key does not depend on the absolute path or on the machine (an extension of t62 to the media as a whole); a foreign derivative does not get into coverage around the cache; opening a damaged store does NOT delete someone else's work |
| Depends on | F-MED-002 (without proxies there is nothing to distribute); related — F-COL-004 (a growing file makes derivatives stale), F-MED-005 (the address of the material), F-PRJ-003 (portability of the project) |
| Breaks compatibility | no |

## Verdict

We do not take it: a unit of distribution requires someone to distribute to, and there is no second participant — the same formula as F-PRJ-003 and F-PRJ-001. The note is opened not for "we take it" but for one requirement on F-MED-002, which is cheap to set in advance and expensive afterwards: **the address and identity of a proxy must be a function of the SOURCE, not of the machine that built the proxy.** The thread about relinking in the shared Production is ready proof of the price of the reverse decision: relinking is needed at every opening, and only for derivatives, and it is fixed by renaming volumes — that is, not by the editor's means at all.

A second thing to write down before implementation: our store of derivatives today is deleted entirely on any opening error. For a personal cache this is correct and should not be changed; but precisely this line separates "cache" from "common property", and if a derivative ever becomes shared, the decision will have to be made anew — here, and not at the moment when someone else's work is already lost.

Boundaries: the proxy toggle and the export protection are F-MED-002; the repair of a path to the material is F-MED-005; a growing source that makes derivatives stale is F-COL-004; moving the whole project to another machine is F-PRJ-003.

## Sources

- Adobe Community, "proxy linking issues in shared production", 24.01.2025 –
  29.01.2025:
  https://community.adobe.com/t5/premiere-pro-discussions/proxy-linking-issues-in-shared-production/td-p/15110876
  (accessed 08.09.2026) — relinking is needed at every opening and only for proxies;
  it is fixed by matching volume names; participants in the thread are J_Boy and
  PaulMurphy (Community Expert).
- Syncthing docs, "Syncing": https://docs.syncthing.net/users/syncing.html
  (08.09.2026) — the rule for resolving a conflict, the name `sync-conflict`,
  "are treated as normal files".
- Blackmagic Design, DaVinci Resolve — Collaboration:
  https://www.blackmagicdesign.com/products/davinciresolve/collaboration
  (08.09.2026) — "As new files are added they will automatically sync and
  appear in the project."
- The distinction between Proxy Media and Optimized Media is quoted from F-MED-002, where it is
  taken from the Resolve installer documentation via the thread on `forum.blackmagicdesign.com`
  (in the current environment the forum is unavailable — 403).
- `forum.blackmagicdesign.com`, threads `t=224635` and `t=214931` — UNAVAILABLE (Cloudflare);
  the fact about `-conflict` files in the proxy folder is taken from search results, marked in
  the text as secondary.
- Live sync mechanism (the camera records an HD H.264 proxy, which is what gets synced) —
  secondary publications from 2026; the exact wording of the primary source could not be
  obtained: **[NOT VERIFIED]**.
- Our code, read on 08.09.2026: `src/apps/kt-app/src/thumbs.rs` (`thumb_key_t`,
  `THUMB_STEP_LADDER`, t62, writing the JPEG to the sidecar DB),
  `src/libs/kt-cache/src/lib.rs` (`BurstKind`, `PostFxVideo`, coverage),
  `src/libs/kt-media/src/index.rs` (`ensure_media`, deleting the DB on an opening error),
  `docs/features/media/proxies/proxy-toggle-workflow.md`.
