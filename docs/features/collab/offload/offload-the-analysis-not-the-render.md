---
id: F-COL-042
title: What to hand out is analysis, not render
status: candidate
priority: B
cost: M
category: collab/offload
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-cache, kt-engine]
invariants: [4, 8]
ui_rules: []
round: null
point: null
tests: [t34, t70]
depends_on: []
trackers: []
services: [frame-io, bm-cloud, premiere-productions]
checked: 2026-09-08
---

# F-COL-042 — What to hand out is analysis, not render

## Essence

Before an edit can begin at all, the program makes several passes over the material, of which the editor does not think: it builds waveforms, cuts thumbnails, remembers where the keyframes lie in the file, brings the sound to a working form. On two hours of material this is tens of minutes of waiting before the first cut. This work is dull, repeatable, and — unlike rendering — gives a TINY result: not gigabytes of pixels, but a few hundred kilobytes of numbers. It is precisely this that is worth handing to someone else: it is portable.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | `.sfk` — a peaks file next to the media | the result of the analysis lies as a SATELLITE of the file in the same folder; it travels with the material by itself, because it is addressed by its name |
| Adobe Premiere | 26.x | Media Cache Files + **Media Cache Database** | the analysis is split in two: heavy files (peaks `.pek`, conformed audio `.cfa`) and a light database mapping the material onto those files. The split is the admission that the expensive and the portable are different things |
| DaVinci Resolve | 21 | Optimized Media, Render Cache, scene analysis | pre-processing is declared an explicit user command and is computed in the background; the result lives in the project's cache directory and is not distributed over the network |
| Final Cut Pro | 12.3 | Background Tasks: analysis, transcode | the analysis stands in one list with import and render and is paused while the person works (F-EXP-002); the library, however, lies whole on a shared volume — that is, the result of the analysis becomes shared AS A SIDE EFFECT |
| After Effects | 26.x | Disk Cache, Conformed audio | the same split "heavy cache against a light map", and the same vendor caveat: the cache is local |

There is no second table here: the mechanism comes from video editing itself, and all five core rows are substantive. The services (`frame-io`, `bm-cloud`) deliberately do not appear in the table — the verdict lives with the editor, and the service does not carry the verdict.

## The trick behind it

**The industry already hands analysis off to the side, only it calls it a cache — and splits it in two exactly along the line of portability.** At Premiere these are two different objects with two different settings: Media Cache **Files** (peaks, conformed audio — gigabytes) and Media Cache **Database** (a map of "which file is served by which cache" — kilobytes). The split is not cosmetic: it means that the vendor has already separated the expensive-but-immovable from the cheap-and-portable. But the next step is not taken, and it is clear why: the key is the PATH, so "sharing the cache" means "sharing the files", that is, carrying those same gigabytes. Hence the support's answer to the question about a shared network cache — "technically feasible but not advisable", and the reasoning in the thread is also about bandwidth, not about correctness.

**Note the wording of the refusal: not "does not work", but "not advisable".** The reason is named in the same thread in plain text — the bottleneck is already the network, and a cache in the network would make it worse. That is, the obstacle is not the mechanism but the DIRECTION of the key: while the analysis result is addressed by path, one can share only files. Had it been addressed by content, one would have to share the database — and it is kilobytes, and the very argument about the 10-gigabit network would simply never arise.

**Our point of receiving someone else's analysis is already written — as a function parameter.** `Source::open_with_index(api, path, prebuilt: Option<PrebuiltIndex>)` (`src/libs/kt-ffmpeg/src/source_impl.rs:132`) accepts a ready result and, if one is present, SKIPS the full scan of the packet stream; the result itself is `PrebuiltIndex { audio_pts: Vec<i64>, video_keyframes: Vec<i64> }` (`src/libs/kt-ffmpeg/src/lib.rs:94`), two arrays of numbers and nothing else. Beside it stands `index_was_prebuilt()` (`src/libs/kt-ffmpeg/src/source_impl.rs:280`) — a function whose only meaning is to distinguish "I scanned the file myself" from "I was handed the answer". This was written for the sake of the local sidecar DB and does not care WHO computed the answer. In other words, the most expensive part of pre-processing already has the form of data rather than of an action — and this is exactly what Premiere does not have.

## Pros

- The result of the analysis is small and grows not as the material does but as its length: peaks, keyframes, points of silence are numbers, not pixels. Transporting them is always cheap; rendering is always expensive.
- The analysis is repeatable and verifiable: the same file gives the same answer, and therefore someone else's answer can be trusted after checking the source — unlike rendering, where one would have to check picture bytes (F-COL-039).
- The analysis is not urgent. It needs neither a frame deadline nor a priority: by definition it is background work, and its lag does not spoil playback.
- The handed-off analysis removes the most noticeable wait — the one that stands BEFORE the work rather than after it. Renders are awaited in the evening, pre-processing in the morning, and the second queue is longer.
- One form of job serves both rendering and analysis (F-COL-038): "command + range + machine requirements" describes equally "compute the frames" and "build the peaks".

## Cons

- Someone else's result requires trust: a wrong packet index gives not an error but silently wrong positions, and it becomes noticeable at the first transition.
- A content key requires reading the content — that is, the first answer to the question "is this the same file" still costs a full pass over the file.
- A shared analysis result means a shared invalidator: a file is overwritten — whose answer is now wrong, and who will find out.
- The split "heavy cache / light map" doubles the number of places where data can diverge, and Premiere already pays for this: restoring the cache after a divergence is a routine procedure for editors.
- An analysis handed to the side returns its result only whole: half of a peak pyramid is useless.

## What users say

A direct question about a shared analysis and a direct answer from the vendor — together they are the finding, because the refusal is phrased as "not advisable" rather than "impossible":

> Can you use a shared network drive to share cache files between multiple
> editors?
> — Pixel Jar, "Media Cache on a shared network drive", Adobe Community,
> 17.03.2021,
> https://community.adobe.com/t5/premiere-pro-discussions/media-cache-on-a-shared-network-drive/td-p/11904757

> It is technically feasible but not advisable according to our senior techs.
> Keep the media cache on local drives.
> — Kevin J. Monahan Jr. (Adobe), same thread, 23.03.2021

The reasoning in the same thread is about bandwidth, not about the mechanism: the network is already the bottleneck, and moving the cache there would only make it worse. This confirms that what stands in the way is not the idea but the key by path: one has to share files, not the map.

`trackers:` is empty deliberately (rule 12): there are no public trackers for the five core editors (`docs/features/_trackers.md`), and the topic "hand pre-processing to another machine" is absent as a class from the twelve exported trackers of open-source NLEs. A number here would be a form without a signal.

## How it fits Kinotrain

### What already exists

- `src/libs/kt-ffmpeg/src/source_impl.rs:132` — `open_with_index` with the parameter `prebuilt`: the point of receiving someone else's result. The comment there says what exactly saves time: "packet index (a full scan without decoding; when a ready index comes from the sidecar DB the scan is skipped)".
- `src/libs/kt-ffmpeg/src/lib.rs:94` — `PrebuiltIndex`: `audio_pts` and `video_keyframes`, two `Vec<i64>`. The whole shape of the result.
- `src/libs/kt-ffmpeg/src/source_impl.rs:280` — `index_was_prebuilt()`: the distinction between "computed it myself" and "was handed the ready one" already exists as a fact of the code, not as a plan.
- `src/libs/kt-engine/src/worker.rs` — the priority queue of MediaWorker: Playback > NearPrefetch > FarPrefetch > **Thumbnail**. Analysis already stands last for us and no longer interferes with playback; handing it outward means removing its tail from this queue, not redesigning the queue.
- `src/libs/kt-media/src/index.rs` — the sidecar `<project>.ktindex.db`: probe, peaks, thumbs, the packet index; xxh3 of the content is already computed on insertion. Tests t34 (persistence of the packet index) and t70 (index_db) guard precisely that the result of the analysis survives the process.
- Invariant 8 — the reason all of the above exists at all: scrolling and zooming must not trigger decoding, so thumbnails and waveforms must be ready in advance. Our pre-processing is not an optimisation but a requirement.
- Invariant 4 — the cache is the single truth about warmth, and `Quality` is part of the key: the discipline "a result is recognised by a key, not by faith" already operates for us.

### What is lacking

- **A key by content instead of a key by path.** The sidecar is keyed by the absolute path, and the file's identity is determined by size and mtime; the xxh3 hash is computed once on insertion and is not the key. This is exactly what distinguishes a portable result from a non-portable one — and it is exactly the mistake for which Premiere pays with the answer "not advisable".
- **A command "compute the analysis and exit".** kt-cli can `render` and `export`, but cannot "build the index/peaks/thumbs for these files": today pre-processing runs only as a side effect of opening a project in the GUI.
- **A way to hand over and receive the result separately from the database.** `PrebuiltIndex` is accepted by a function, but as a self-standing thing (a file, a string, anything) it is not expressed; it has no serialisation.
- **A list of what counts as analysis at all.** The packet index, peaks and thumbs live in the sidecar, but the silence detection (F-AUD-008), the noise profile (F-AUD-003) and the integrated loudness (F-AUD-005) do not exist at all — although all three belong to the same class and were already named in the first wave as blocked by one missing subsystem.

### What stands in the way

Nothing constructive — and this is a rare case in the category. The point of receiving is written, the shape of the result is trivial, the queue already places analysis last, invariant 8 requires that analysis exist. One thing stands in the way, and it is not about the network: **the result is addressed by path, not by content.** As long as this is so, someone else's answer cannot be recognised as belonging to my file, even if it lies right beside it. The fix costs little (xxh3-128 is already computed there), but it is a change of the key of an existing database, that is, a migration, and it must be done deliberately.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/libs/kt-ffmpeg/src/source_impl.rs:132` — the trick is fully expressible on one machine: `open_with_index` accepts a ready analysis result as a parameter and skips the full scan of the packet stream, `index_was_prebuilt` distinguishes "computed it myself" from "was handed the ready one", and the result itself is two arrays of numbers; all of this was written for the sake of the local sidecar DB, to which it does not matter who exactly computed it.

NETWORK COST: a foreign machine that computed the index, and a common place where it lies for several projects — cross-project storage — the sidecar is tied to one project and keyed by the absolute path, so someone else's answer cannot be recognised as belonging to my file, although the content hash next to it is already computed.

### Which invariants it touches

Invariant 8 (scrolling and zooming do not produce decoding; thumbnails and waveforms come from the sidecar index) — the feature IS a consequence of this invariant, not an encroachment on it. Invariant 4 (the cache is the single truth about warmth, `Quality` is part of the key) — a model of discipline to be extended to the sidecar: the result is recognised by a key. Invariant 5 (one MediaWorker owns the decoder) is not touched: an analysis computed outside arrives as data, not as a second owner of the decoder. Rules of the UI are not touched by this note.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages: a content key in the sidecar (with a migration) and a headless command "compute the analysis" |
| Where the main work is | kt-media (the key of an index row — a content hash, not the absolute path; migration of existing DBs), kt-cli (the pre-processing command), kt-ffmpeg (`PrebuiltIndex` as a self-standing thing) |
| Project format | not touched: the sidecar is not the project format |
| Tests to write | "a file moved to another folder is recognised by content and is not scanned again" (a pair to t34); "a sidecar from a foreign project is accepted for the same file"; "a result with a non-matching hash is rejected, not applied silently"; migration of an old DB keyed by path into a DB keyed by content |
| Depends on | nothing mandatory; by the conclusion of the first wave, F-AUD-003, F-AUD-005, F-AUD-008, F-SPD-004, F-MSK-005 also adjoin here — all five ask for one and the same subsystem of offline analysis |
| Breaks compatibility | no for projects; for existing `.ktindex.db` — a migration (on any error opening the DB it is today already recreated) |

## Verdict

We take it — and this is the only note in the subcategory where there is something to take right now. The local core: **the key of an analysis result must be by CONTENT, not by path.** This one fix makes pre-processing portable — between folders, between projects, and, if a second participant ever appears, between machines — and it is useful immediately, without any second participant: a file moved to another folder will stop being rescanned. The second step is a headless command "compute the analysis and exit", which turns kt-cli into a ready executor of pre-processing on the same machine, at night, from a script.

We do not set up the server half — handing results to foreign machines — because there is no second participant, and this note has no right to declare its appearance. But the note is worth remembering for another reason: **this is the only subject in the whole subcategory for which sending has any meaning at all.** A render has nowhere to be sent (F-COL-041), pieces have nothing to assemble them from (F-COL-040), but the analysis is small, repeatable, and already has an entry point in us. If the queue ever leaves the machine, it is this that will leave.

Boundaries: where the sidecar lies and what happens to it on a shared volume — F-COL-002; proxies as the second portable product of analysis — F-MED-002; the form of the job — F-COL-038; the subsystem of offline analysis as the common denominator of the five features — the conclusion of the first wave in `docs/features/README.md`.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/media-cache-on-a-shared-network-drive/td-p/11904757 — the question by Pixel Jar (17.03.2021) and the answer by Kevin J. Monahan Jr. (23.03.2021), as well as the reasoning about bandwidth in the replies of basil1891 and Jeff Bugbee of the same day (access date 2026-09-08)
- https://superrendersfarm.com/article/how-render-farms-work-technical-guide — context: what a farm counts as a unit of work and why encoding is not divided (published 04.04.2026, updated 07.09.2026; 2026-09-08)
- https://docs.opencue.io/docs/concepts/command-execution/ — the form of a farm job describes a render and an arbitrary command alike: "Resources are allocated per-frame, so all commands in a chunk share the same resources" (2026-09-08)
- The rows for Resolve, Final Cut and After Effects in the table rely on the composition of their background tasks and cache settings; they were not verified live, and the five have no public tracker (`docs/features/_trackers.md`, "Whom we do NOT take").
  **[NOT VERIFIED]** against the primary source: the exact composition of VEGAS's `.sfk` and the location of Resolve's cache directory — `helpx.adobe.com` and `forum.blackmagicdesign.com` are unavailable from this environment (`docs/features/_services.md`, the availability table).
- Our code checked against the tree on 08.09.2026: `src/libs/kt-ffmpeg/src/source_impl.rs` (132, 280), `src/libs/kt-ffmpeg/src/lib.rs:94`, `src/libs/kt-engine/src/worker.rs`, `src/libs/kt-media/src/index.rs`; tests `t34` (`ffmpeg_conformance.rs`) and `t70` (`index_db.rs`).
